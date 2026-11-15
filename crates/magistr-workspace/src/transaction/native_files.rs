use super::{TransactionError, snapshot::Snapshot};
use crate::{
    file_content::FileContent, source_open::open_source, workspace_policy::WorkspacePolicy,
};
#[cfg(not(unix))]
use std::io::Write;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(super) struct NativeFiles {
    pub root: PathBuf,
}
impl NativeFiles {
    pub fn resolve(
        &self,
        policy: &WorkspacePolicy,
        path: &str,
    ) -> Result<PathBuf, TransactionError> {
        let admitted = policy.check_write(path)?;
        if admitted == self.root {
            return Err(TransactionError::InvalidInput(
                "cannot replace the workspace root".into(),
            ));
        }
        Ok(admitted)
    }

    pub fn snapshot(
        &self,
        policy: &WorkspacePolicy,
        path: &str,
        expected_path: &Path,
    ) -> Result<Snapshot, TransactionError> {
        if self.resolve(policy, path)? != expected_path {
            return Err(TransactionError::StaleAnchor(format!(
                "{path}: path resolution changed"
            )));
        }
        self.read_snapshot(path, expected_path)
    }

    pub fn baseline(
        &self,
        policy: &WorkspacePolicy,
        path: &str,
        expected_path: &Path,
    ) -> Result<Snapshot, TransactionError> {
        if policy.check_read(path)? != expected_path {
            return Err(TransactionError::StaleAnchor(path.into()));
        }
        self.read_snapshot(path, expected_path)
    }

    fn read_snapshot(
        &self,
        path: &str,
        expected_path: &Path,
    ) -> Result<Snapshot, TransactionError> {
        let metadata = match fs::symlink_metadata(expected_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Snapshot {
                    content: None,
                    permissions: None,
                });
            }
            Err(error) => return Err(io(error)),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(TransactionError::Io(format!(
                "{path}: expected a regular file"
            )));
        }
        if metadata.len() > 4 * 1024 * 1024 {
            return Err(TransactionError::InvalidInput(format!(
                "{path}: source exceeds 4 MiB"
            )));
        }
        let file = open_source(&self.root, expected_path)?;
        let permissions = file.metadata().map_err(io)?.permissions();
        let mut bytes = Vec::new();
        file.take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(TransactionError::InvalidInput(format!(
                "{path}: source exceeds 4 MiB"
            )));
        }
        Ok(Snapshot {
            content: Some(FileContent::decode(bytes)),
            permissions: Some(permissions),
        })
    }

    /// Each replacement is atomic; an error after rename (for example directory
    /// sync) may nevertheless have changed the file and requires read-back.
    /// Missing parents of a written file are created on the admitted path
    /// first and appended to `created` (outermost first) before any swap, so
    /// a failure after creation still leaves them recorded for compensation.
    pub fn replace(
        &self,
        policy: &WorkspacePolicy,
        path: &str,
        admitted: &Path,
        content: Option<&FileContent>,
        permissions: Option<&fs::Permissions>,
        created: &mut Vec<PathBuf>,
    ) -> Result<(), TransactionError> {
        if self.resolve(policy, path)? != admitted {
            return Err(TransactionError::StaleAnchor(path.into()));
        }
        let parent = admitted
            .parent()
            .ok_or_else(|| TransactionError::Io("missing target parent".into()))?;
        if content.is_some() {
            created.extend(
                super::created_parents::create_missing_parents(&self.root, parent).map_err(io)?,
            );
        }
        #[cfg(unix)]
        {
            super::unix_swap::replace(&self.root, admitted, content, permissions).map_err(io)
        }
        #[cfg(not(unix))]
        {
            if let Some(content) = content {
                let mut staged = StagedFile::beside(parent).map_err(io)?;
                staged.file.write_all(content.as_bytes()).map_err(io)?;
                if let Some(permissions) = permissions {
                    staged
                        .file
                        .set_permissions(permissions.clone())
                        .map_err(io)?;
                }
                staged.file.sync_all().map_err(io)?;
                if self.resolve(policy, path)? != admitted {
                    return Err(TransactionError::StaleAnchor(path.into()));
                }
                staged.publish(admitted).map_err(io)?;
            } else {
                let trash = super::trash::directory(&self.root).map_err(io)?;
                let trashed = trash.join(super::trash::entry_name(admitted).map_err(io)?);
                fs::rename(admitted, trashed).map_err(io)?;
            }
            Ok(())
        }
    }
}
/// A sibling of the file being replaced, renamed over it once its content
/// is durable and removed if that never happens. Every step goes through the
/// standard library, which accepts a source file at any depth; a rename made
/// through the operating system's own interface refuses a Windows path
/// longer than 260 characters.
#[cfg(not(unix))]
struct StagedFile {
    path: PathBuf,
    file: fs::File,
    published: bool,
}

#[cfg(not(unix))]
impl StagedFile {
    fn beside(parent: &Path) -> std::io::Result<Self> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = parent.join(format!(
                ".tmp-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn publish(mut self, target: &Path) -> std::io::Result<()> {
        fs::rename(&self.path, target)?;
        self.published = true;
        Ok(())
    }
}

#[cfg(not(unix))]
impl Drop for StagedFile {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn io(error: std::io::Error) -> TransactionError {
    TransactionError::Io(error.to_string())
}

#[cfg(test)]
#[path = "native_files.tests.rs"]
mod tests;
