use std::{fs, io::Read, path::PathBuf};

use crate::{
    contracts::{ErrorKind, WorkspaceError},
    file_content::FileContent,
    source_open::open_source,
    workspace_policy::WorkspacePolicy,
};

/// Admitted, bounded reads used to resolve a verb. Every value observed here is
/// re-anchored by hash in the transaction, so a raced change fails closed.
pub(super) struct SourceReads<'a> {
    policy: &'a WorkspacePolicy,
    root: PathBuf,
}

impl<'a> SourceReads<'a> {
    pub fn new(policy: &'a WorkspacePolicy) -> Result<Self, WorkspaceError> {
        Ok(Self {
            policy,
            root: policy.check_read(".")?,
        })
    }

    /// The workspace-relative posix dialect shared with the retained tools.
    pub fn normalize(path: &str) -> String {
        path.replace('\\', "/")
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>()
            .join("/")
    }

    /// `Some` bytes for an existing regular file, `None` when absent. Policy
    /// refusals, directories and oversized files are errors.
    pub fn read(&self, path: &str) -> Result<Option<FileContent>, WorkspaceError> {
        let admitted = self.policy.check_read(path)?;
        let metadata = match fs::symlink_metadata(&admitted) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(WorkspaceError::new(
                    ErrorKind::FileNotFound,
                    format!("{path}: {error}"),
                ));
            }
        };
        if !metadata.is_file() {
            return Err(WorkspaceError::new(
                ErrorKind::FileNotFound,
                format!(
                    "{path:?} is not a regular file; directories are not movable, copyable or editable in v1, only files"
                ),
            ));
        }
        if metadata.len() > 4 * 1024 * 1024 {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                format!("{path}: source exceeds the 4 MiB mutation bound"),
            ));
        }
        let file = open_source(&self.root, &admitted)?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                WorkspaceError::new(ErrorKind::FileNotFound, format!("{path}: {error}"))
            })?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                format!("{path}: source exceeds the 4 MiB mutation bound"),
            ));
        }
        Ok(Some(FileContent::decode(bytes)))
    }

    /// The hash the retained STALE_ANCHOR payload carries: the current bytes,
    /// or the hash of empty bytes when the file is absent.
    pub fn current_hash(&self, path: &str) -> String {
        self.read(path)
            .ok()
            .flatten()
            .unwrap_or_else(|| FileContent::Text(String::new()))
            .content_hash()
    }
}
