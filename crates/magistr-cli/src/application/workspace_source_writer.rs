use std::path::Path;
use std::sync::Arc;

use magistr_application::effects::{EffectError, EffectFuture, SourceWriteReceipt, SourceWriter};
use magistr_workspace::{
    commands::WriteInput,
    contracts::{ErrorKind, WorkspaceError, WorkspaceResponse},
    service::{WorkspaceService, WorkspaceSession},
};

/// Magistr's own source writes over the workspace this process admitted:
/// one explicit journal session for the run, every write through the
/// service's `write` verb — the shared pipeline, so the intent is journaled
/// and the answer is the resolved [`MutationReceipt`]'s revision, never the
/// sequence. Refusals (a protected path, a rejected parse, a stale anchor)
/// come back as typed effect errors in the pipeline's own words.
///
/// [`MutationReceipt`]: magistr_workspace::snapshot::MutationReceipt
pub struct WorkspaceSourceWriter {
    service: Arc<WorkspaceService>,
    session: WorkspaceSession,
}

impl WorkspaceSourceWriter {
    /// A writer over `service` under a fresh explicit session identity.
    ///
    /// # Errors
    /// The service refused to mint a session.
    pub fn new(service: Arc<WorkspaceService>) -> Result<Self, WorkspaceError> {
        let session = service.new_session()?;
        Ok(Self { service, session })
    }

    /// The journal identity this writer's ops are recorded under.
    #[must_use]
    pub fn session_id(&self) -> &str {
        self.session.id()
    }

    async fn write(&self, path: &str, content: &str) -> Result<SourceWriteReceipt, EffectError> {
        let relative = relative_to(self.service.root(), path)?;
        let input = WriteInput::new(relative, content);
        match self.service.write(&input, &self.session).await {
            WorkspaceResponse::Success { results, .. } => {
                let Some(row) = results.rows().first() else {
                    return Ok(SourceWriteReceipt {
                        journal_seq: None,
                        revision: None,
                        changed: false,
                    });
                };
                if !row.applied {
                    return Err(EffectError::Failed(format!(
                        "write {path}: the pipeline reported {}",
                        row.disposition
                    )));
                }
                let revision = match row.journal_seq {
                    Some(seq) => self
                        .service
                        .mutation_receipt(seq)
                        .await
                        .map_err(|error| EffectError::Failed(error.message))?
                        .and_then(|receipt| receipt.revision),
                    None => None,
                };
                Ok(SourceWriteReceipt {
                    journal_seq: row.journal_seq,
                    revision,
                    changed: true,
                })
            }
            WorkspaceResponse::Failure { error, notes, .. } => {
                let mut message = format!("write {path}: {}", error.message);
                if let Some(notes) = notes
                    && !notes.is_empty()
                {
                    message.push_str(&format!(" ({})", notes.join("; ")));
                }
                Err(match error.kind {
                    ErrorKind::ProtectedPath => EffectError::Denied(message),
                    _ => EffectError::Failed(message),
                })
            }
        }
    }
}

impl SourceWriter for WorkspaceSourceWriter {
    fn write_text(&self, path: &str, content: &str) -> EffectFuture<'_, SourceWriteReceipt> {
        let path = path.to_owned();
        let content = content.to_owned();
        Box::pin(async move { self.write(&path, &content).await })
    }
}

/// The workspace-relative posix path of an absolute `path` under `root`;
/// a path outside the admitted root is denied rather than resolved.
fn relative_to(root: &Path, path: &str) -> Result<String, EffectError> {
    let relative = Path::new(path).strip_prefix(root).map_err(|_| {
        EffectError::Denied(format!(
            "{path} is outside the admitted workspace {}",
            root.display()
        ))
    })?;
    let parts = relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| EffectError::InvalidRequest("path is not UTF-8".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.is_empty() {
        return Err(EffectError::InvalidRequest(
            "the workspace root itself is not a file".into(),
        ));
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
#[path = "workspace_source_writer.tests.rs"]
mod tests;
