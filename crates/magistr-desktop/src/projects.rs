//! Desktop review handles retain application-owned setup intent; the window
//! translates the administrative plan and its actual receipt.
use magistr::{
    application::{ProjectFolder, ProjectSetup},
    config::inputs::ProjectInputs,
    scaffold::Ownership,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
pub(crate) struct ProjectReviews {
    issued: AtomicU64,
    current: Mutex<Option<Review>>,
}
struct Review {
    id: u64,
    workspace: PathBuf,
    setup: ProjectSetup,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProjectPreview {
    Ready {
        directory: PathBuf,
    },
    Setup {
        review: u64,
        directory: PathBuf,
        setup_directory: PathBuf,
        files: Vec<PlannedFile>,
    },
}
#[derive(Debug, Serialize)]
pub(crate) struct PlannedFile {
    pub(crate) path: String,
    action: &'static str,
    ownership: &'static str,
    reason: Option<String>,
    pub(crate) selectable: bool,
    before: Option<String>,
    after: String,
    diff: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct SetupApplied {
    pub(crate) complete: bool,
    directory: PathBuf,
    pub(crate) files: Vec<AppliedFile>,
    setup_record: String,
}
#[derive(Serialize)]
pub(crate) struct AppliedFile {
    path: String,
    pub(crate) outcome: String,
    detail: Option<String>,
}
impl ProjectReviews {
    pub(crate) fn preview(
        &self,
        directory: &Path,
        inputs: &ProjectInputs,
        review: bool,
    ) -> Result<ProjectPreview, String> {
        let id = self.issued.fetch_add(1, Ordering::SeqCst) + 1;
        *self.current.lock().map_err(|_| review_error())? = None;
        let folder = ProjectFolder::inspect(directory).map_err(|error| error.to_string())?;
        let workspace = folder.directory;
        if folder.prepared && !review {
            return Ok(ProjectPreview::Ready {
                directory: workspace,
            });
        }
        let setup = ProjectSetup::preview(&workspace, inputs).map_err(|error| error.to_string())?;
        let files = setup
            .plan()
            .files
            .iter()
            .map(|file| PlannedFile {
                path: file.path.clone(),
                action: match file.action {
                    magistr::scaffold::FileAction::Create => "Create file",
                    magistr::scaffold::FileAction::Overwrite => "Update file",
                    magistr::scaffold::FileAction::Remove => "Remove file",
                    magistr::scaffold::FileAction::Unchanged => "No change",
                    magistr::scaffold::FileAction::Preserved => "Keep file",
                    magistr::scaffold::FileAction::Refused(_) => "Cannot change",
                },
                ownership: ownership(file.ownership),
                reason: file.reason(),
                selectable: file.action.is_applied(),
                before: file.existing_content.clone(),
                after: file.new_content.clone(),
                diff: file.diff.clone(),
            })
            .collect();
        let preview = ProjectPreview::Setup {
            review: id,
            directory: workspace.clone(),
            setup_directory: setup.directory().to_path_buf(),
            files,
        };
        let mut current = self.current.lock().map_err(|_| review_error())?;
        if self.issued.load(Ordering::SeqCst) != id {
            return Err(review_error());
        }
        *current = Some(Review {
            id,
            workspace,
            setup,
        });
        Ok(preview)
    }
    pub(crate) fn apply(&self, review: u64, approved: &[usize]) -> Result<SetupApplied, String> {
        let review = {
            let mut current = self.current.lock().map_err(|_| review_error())?;
            if current.as_ref().is_none_or(|value| value.id != review) {
                return Err(review_error());
            }
            current.take().ok_or_else(review_error)?
        };
        let result = review
            .setup
            .apply(approved)
            .map_err(|error| error.to_string())?;
        Ok(SetupApplied {
            complete: result.complete,
            directory: review.workspace,
            files: result
                .receipt
                .changes
                .iter()
                .map(|file| AppliedFile {
                    path: file.path.clone(),
                    outcome: file.outcome.label().into(),
                    detail: file.outcome.detail(),
                })
                .collect(),
            setup_record: match result.receipt.manifest {
                magistr::scaffold::ManifestOutcome::Untouched => "Setup record unchanged.".into(),
                magistr::scaffold::ManifestOutcome::Saved => "Setup record saved.".into(),
                magistr::scaffold::ManifestOutcome::Failed { cause } => {
                    format!("Could not save the setup record: {cause}")
                }
            },
        })
    }
    pub(crate) fn discard(&self, review: u64) -> Result<(), String> {
        let mut current = self.current.lock().map_err(|_| review_error())?;
        if current.as_ref().is_some_and(|value| value.id == review) {
            *current = None;
        }
        Ok(())
    }
}
fn review_error() -> String {
    "This setup review has ended. Review the folder again.".into()
}
fn ownership(value: Ownership) -> &'static str {
    match value {
        Ownership::Unclaimed => "New file",
        Ownership::Internal | Ownership::Generated => "Managed by Magistr",
        Ownership::Customized => "Your edited file",
        Ownership::User => "Your file",
        Ownership::Shared => "Shared file",
        Ownership::Canonical => "Kept by this project",
    }
}
#[cfg(test)]
#[path = "projects.tests.rs"]
mod tests;
