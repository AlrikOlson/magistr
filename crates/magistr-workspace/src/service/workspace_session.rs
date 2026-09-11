use std::path::{Path, PathBuf};

use super::WorkspaceGeneration;
use crate::contracts::{ErrorKind, WorkspaceError};

/// An explicit session admitted by one workspace owner generation.
///
/// Only the service can mint a session. Its private admission fields prevent a
/// caller from retargeting a handle to another workspace or replacement owner.
/// The id remains the journal identity across deliberate re-admission, while
/// the generation fences handles retained from a previous owner.
#[derive(Debug, Clone)]
pub struct WorkspaceSession {
    root: PathBuf,
    generation: WorkspaceGeneration,
    id: String,
}

impl WorkspaceSession {
    /// The retained ceiling counts UTF-16 units, including two per astral code
    /// point, so explicit identities cannot grow the journal without bound.
    const MAX_ID_UNITS: usize = crate::response::ResponseAdapter::SESSION_ID_MAX_UNITS;

    pub(super) fn new(
        root: PathBuf,
        generation: WorkspaceGeneration,
        id: String,
    ) -> Result<Self, WorkspaceError> {
        if id.trim().is_empty() || id.encode_utf16().count() > Self::MAX_ID_UNITS {
            return Err(WorkspaceError::new(
                ErrorKind::ConfigPolicyDenied,
                "workspace session identity must contain a non-whitespace character and fit within 256 UTF-16 units; supply an explicit session identity before admission",
            ));
        }
        Ok(Self {
            root,
            generation,
            id,
        })
    }

    /// The durable identity used for journal provenance and session-scoped undo.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The owner incarnation that admitted this handle.
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }

    /// Validate against the service's already canonical root and current owner
    /// generation before entering a workspace use case.
    pub(super) fn validate(
        &self,
        root: &Path,
        generation: WorkspaceGeneration,
    ) -> Result<(), WorkspaceError> {
        if self.root != root {
            return Err(WorkspaceError::new(
                ErrorKind::EngineUnavailable,
                "workspace session belongs to another workspace; obtain a session from this workspace owner before retrying",
            ));
        }
        if self.generation != generation {
            return Err(WorkspaceError::new(
                ErrorKind::EngineUnavailable,
                format!(
                    "workspace session belongs to owner generation {}, but the current owner is generation {}; re-admit the session before retrying",
                    self.generation.get(),
                    generation.get(),
                ),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "workspace_session.tests.rs"]
mod tests;
