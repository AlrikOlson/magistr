//! Working-copy lifecycle joins Git's registry, live ownership and source status.
//! These records and decisions change together; presentation keeps no second registry.
use crate::orchestration::ledger::GitRepo;
use magistr_engine::native::{ProjectLocation, WorkingCopyLease};
use magistr_workspace::service::ServerLock;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WorkingCopyState {
    Available { dirty: u32, conflicts: u32 },
    Archived,
    Locked { reason: String },
    Unavailable,
}
#[derive(Debug, Clone, Serialize)]
pub struct WorkingCopy {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub is_main: bool,
    pub state: WorkingCopyState,
}
#[derive(Debug)]
pub enum WorkingCopyError {
    Active,
    Dirty,
    Conflicts,
    Unavailable,
    Foreign,
    Main,
    Locked(String),
    Operation(String),
}

impl std::fmt::Display for WorkingCopyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => {
                f.write_str("Work is still open in this working copy. Close it before archiving.")
            }
            Self::Dirty => f.write_str("Review or save the local changes before archiving."),
            Self::Conflicts => f.write_str("Resolve the merge conflicts before archiving."),
            Self::Unavailable => f.write_str(
                "This working copy is unavailable. Restore its directory before opening it.",
            ),
            Self::Foreign => f.write_str("This working copy belongs to another project."),
            Self::Main => f.write_str("The main working copy stays open."),
            Self::Locked(reason) => write!(f, "This working copy is locked: {reason}"),
            Self::Operation(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for WorkingCopyError {}

/// One application entry point for every current working-copy operation.
pub struct WorkingCopies {
    repo: GitRepo,
    root: PathBuf,
}

impl WorkingCopies {
    pub fn at(root: &Path) -> Result<Self, WorkingCopyError> {
        let location = ProjectLocation::resolve(root).map_err(operation)?;
        let root =
            ProjectLocation::working_copy_root(&location.authority_root).map_err(operation)?;
        Ok(Self {
            repo: GitRepo::at(&root),
            root,
        })
    }

    pub fn list(&self) -> Result<Vec<WorkingCopy>, WorkingCopyError> {
        let records = self
            .repo
            .working_copies()
            .map_err(WorkingCopyError::Operation)?;
        let mut copies = Vec::new();
        let mut path = None;
        let mut branch = None;
        let mut locked = None;
        for field in records.split('\0') {
            if let Some(value) = field.strip_prefix("worktree ") {
                path = Some(PathBuf::from(value));
            } else if let Some(value) = field.strip_prefix("branch refs/heads/") {
                branch = Some(value.to_owned());
            } else if field == "locked" {
                locked = Some(String::new());
            } else if let Some(value) = field.strip_prefix("locked ") {
                locked = Some(value.to_owned());
            } else if field.is_empty()
                && let Some(path) = path.take()
            {
                let state = if !path.is_dir() {
                    WorkingCopyState::Unavailable
                } else if let Some(reason) = locked.take() {
                    if reason == ProjectLocation::ARCHIVE_REASON {
                        WorkingCopyState::Archived
                    } else {
                        WorkingCopyState::Locked { reason }
                    }
                } else {
                    let (dirty, conflicts) = GitRepo::at(&path)
                        .changes()
                        .map_err(WorkingCopyError::Operation)?;
                    WorkingCopyState::Available { dirty, conflicts }
                };
                copies.push(WorkingCopy {
                    is_main: path == self.root,
                    path,
                    branch: branch.take(),
                    state,
                });
                locked = None;
            }
        }
        Ok(copies)
    }

    fn copy(&self, path: &Path) -> Result<WorkingCopy, WorkingCopyError> {
        let absolute = std::path::absolute(path).map_err(operation)?;
        let path = absolute
            .canonicalize()
            .or_else(|_| {
                let parent = absolute
                    .parent()
                    .ok_or_else(|| std::io::Error::other("Missing parent directory."))?;
                let name = absolute
                    .file_name()
                    .ok_or_else(|| std::io::Error::other("Missing directory name."))?;
                Ok::<_, std::io::Error>(parent.canonicalize()?.join(name))
            })
            .map_err(operation)?;
        self.list()?
            .into_iter()
            .find(|copy| copy.path == path)
            .ok_or(WorkingCopyError::Foreign)
    }

    pub fn create(
        &self,
        path: &Path,
        branch: &str,
        head: &str,
    ) -> Result<WorkingCopy, WorkingCopyError> {
        self.repo
            .create_working_copy(path, branch, head)
            .map_err(WorkingCopyError::Operation)?;
        self.copy(path)
    }

    /// Archiving preserves the working tree, branch, source journal and execution history.
    pub fn archive(&self, path: &Path) -> Result<WorkingCopy, WorkingCopyError> {
        let copy = self.copy(path)?;
        if copy.is_main {
            return Err(WorkingCopyError::Main);
        }
        if copy.state == WorkingCopyState::Unavailable {
            return Err(WorkingCopyError::Unavailable);
        }
        let (_work, _source) = freeze(&copy.path)?;
        let (dirty, conflicts) = GitRepo::at(&copy.path)
            .changes()
            .map_err(WorkingCopyError::Operation)?;
        if conflicts != 0 {
            return Err(WorkingCopyError::Conflicts);
        }
        if dirty != 0 {
            return Err(WorkingCopyError::Dirty);
        }
        match &copy.state {
            WorkingCopyState::Archived => return Ok(copy),
            WorkingCopyState::Locked { reason } => {
                return Err(WorkingCopyError::Locked(reason.clone()));
            }
            _ => {}
        }
        self.repo
            .archive_working_copy(&copy.path)
            .map_err(WorkingCopyError::Operation)?;
        self.copy(&copy.path)
    }

    pub fn open(&self, path: &Path) -> Result<WorkingCopy, WorkingCopyError> {
        let copy = self.copy(path)?;
        match &copy.state {
            WorkingCopyState::Unavailable => Err(WorkingCopyError::Unavailable),
            WorkingCopyState::Locked { reason } => Err(WorkingCopyError::Locked(reason.clone())),
            WorkingCopyState::Archived => {
                let (_work, _source) = freeze(&copy.path)?;
                self.repo
                    .reopen_working_copy(&copy.path)
                    .map_err(WorkingCopyError::Operation)?;
                self.copy(&copy.path)
            }
            _ => Ok(copy),
        }
    }
}

fn operation(error: std::io::Error) -> WorkingCopyError {
    if error.kind() == std::io::ErrorKind::WouldBlock {
        WorkingCopyError::Active
    } else {
        WorkingCopyError::Operation(error.to_string())
    }
}

fn freeze(root: &Path) -> Result<(WorkingCopyLease, ServerLock), WorkingCopyError> {
    let work = WorkingCopyLease::archive(root).map_err(operation)?;
    std::fs::create_dir_all(root.join(".workbench")).map_err(operation)?;
    let source = ServerLock::acquire(root.join(".workbench/server.lock")).map_err(operation)?;
    Ok((work, source))
}

#[cfg(test)]
#[path = "working_copies.tests.rs"]
mod tests;
