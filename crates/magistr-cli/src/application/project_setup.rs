//! A reviewed project setup and its completion decision share the administrative
//! plan and receipt. The application holds intent; AdminService owns every write.
use crate::config::inputs::ProjectInputs;
use crate::error::{MagistrError, Result};
use crate::scaffold::{
    ApplyOutcome, ApplyReceipt, ChangePlan, ChangeReceipt, FileAction, pipeline::ScaffoldPipeline,
};
use std::path::{Path, PathBuf};

/// Read-only readiness shared by project previews and session admission.
pub struct ProjectFolder {
    pub directory: PathBuf,
    pub prepared: bool,
}
impl ProjectFolder {
    pub fn inspect(directory: &Path) -> Result<Self> {
        let directory = directory.canonicalize().map_err(|error| {
            MagistrError::Other(format!(
                "Could not open folder {}: {error}",
                directory.display()
            ))
        })?;
        if !directory.is_dir() {
            return Err(MagistrError::DirNotFound(directory));
        }
        let settings = crate::paths::project_json(&directory)?;
        let prepared = match std::fs::symlink_metadata(&settings) {
            Ok(_) => {
                crate::validate::load_project_config(&directory)?;
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(MagistrError::io(&settings, error)),
        };
        Ok(Self {
            directory,
            prepared,
        })
    }
}

pub struct ProjectSetup {
    directory: PathBuf,
    plan: ChangePlan,
}
pub struct SetupResult {
    pub receipt: ApplyReceipt,
    pub complete: bool,
}
impl ProjectSetup {
    /// Preview the current owner's setup without writing or launching an agent.
    pub fn preview(directory: &Path, inputs: &ProjectInputs) -> Result<Self> {
        let workspace = ProjectFolder::inspect(directory)?.directory;
        let directory = magistr_engine::native::ProjectLocation::resolve(&workspace)
            .map_err(|error| MagistrError::io(&workspace, error))?
            .authority_root;
        let inputs = inputs.for_project(&directory);
        let tree = crate::detect::build_project_tree(&directory)?;
        let pipeline = if tree.len() > 1 {
            ScaffoldPipeline::polyglot(&tree)
        } else {
            ScaffoldPipeline::headless(crate::detect::detect_stack_full(&directory, &inputs)?)
        }
        .with_inputs(&inputs);
        let plan = pipeline.preview(&directory)?;
        Ok(Self { directory, plan })
    }
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    #[must_use]
    pub fn plan(&self) -> &ChangePlan {
        &self.plan
    }
    /// Apply this exact preview. Declined paths and failures keep setup incomplete.
    pub fn apply(&self, approved: &[usize]) -> Result<SetupResult> {
        let mut selected = vec![false; self.plan.files.len()];
        for &index in approved {
            let Some(entry) = selected.get_mut(index) else {
                return Err(MagistrError::Other(
                    "This setup selection is no longer valid. Review setup again.".into(),
                ));
            };
            *entry = true;
        }
        let files: Vec<_> = self
            .plan
            .files
            .iter()
            .zip(&selected)
            .filter_map(|(file, &selected)| selected.then_some(file))
            .collect();
        let mut receipt = crate::scaffold::apply::apply_approved(&self.directory, &files)?;
        let mut applied = std::mem::take(&mut receipt.changes).into_iter();
        receipt.changes = self
            .plan
            .files
            .iter()
            .zip(&selected)
            .map(|(file, &selected)| {
                if selected && file.action.is_applied() {
                    // apply_approved records selected mutations in the same plan order.
                    applied.next().expect(
                        "the administrative owner returns one receipt per selected mutation",
                    )
                } else {
                    let outcome = match &file.action {
                        FileAction::Refused(reason) => ApplyOutcome::Refused {
                            reason: reason.to_string(),
                        },
                        _ => ApplyOutcome::Skipped {
                            reason: file.reason().unwrap_or_else(|| "Not selected.".into()),
                        },
                    };
                    ChangeReceipt::new(file, outcome)
                }
            })
            .collect();
        let complete = !receipt.is_partial()
            && self
                .plan
                .files
                .iter()
                .zip(&selected)
                .all(|(file, &selected)| {
                    !matches!(file.action, FileAction::Refused(_))
                        && (!file.action.is_applied() || selected)
                })
            && crate::validate::load_project_config(&self.directory).is_ok();
        Ok(SetupResult { receipt, complete })
    }
}
#[cfg(test)]
#[path = "project_setup.tests.rs"]
mod tests;
