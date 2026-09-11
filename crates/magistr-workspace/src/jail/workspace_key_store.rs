use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::{KeyError, WorkspaceKey};

/// Key lifecycle for a trusted workspace owner's state directory.
///
/// The owner supplies the state directory; this is not a tool-addressed write.
/// Key directories and targets refuse symlinks. Fresh material is installed
/// without overwriting a concurrent initialization. Rotation replaces atomically.
/// New key directories use POSIX 0700 and generated keys use 0600. Existing
/// directory/key modes are preserved on load, as in the reference. Windows
/// protection relies on the directory ACL.
/// File contents are synchronized before installation, but directory durability
/// and hostile external directory swaps require the later workspace owner.
pub struct WorkspaceKeyStore {
    state_dir: PathBuf,
}

impl WorkspaceKeyStore {
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.state_dir.join("keys/workspace.key")
    }

    pub fn ensure(&self) -> Result<WorkspaceKey, KeyError> {
        self.prepare()?;
        match self.load() {
            Ok(key) => return Ok(key),
            Err(KeyError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let (key, file) = self.fresh_file()?;
        match file.persist_noclobber(self.path()) {
            Ok(_) => Ok(key),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => self.load(),
            Err(error) => Err(io_error(&self.path(), error.error)),
        }
    }

    pub fn rotate(&self) -> Result<WorkspaceKey, KeyError> {
        self.prepare()?;
        check_regular_target(&self.path())?;
        let (key, file) = self.fresh_file()?;
        file.persist(self.path())
            .map_err(|error| io_error(&self.path(), error.error))?;
        Ok(key)
    }

    fn prepare(&self) -> Result<(), KeyError> {
        ensure_directory(&self.state_dir)?;
        ensure_directory(&self.state_dir.join("keys"))
    }

    fn load(&self) -> Result<WorkspaceKey, KeyError> {
        let path = self.path();
        check_regular_target(&path)?;
        let file = std::fs::File::open(&path).map_err(|error| io_error(&path, error))?;
        let size = file
            .metadata()
            .map_err(|error| io_error(&path, error))?
            .len();
        if size != 32 {
            return Err(KeyError::Malformed {
                path,
                length: usize::try_from(size).unwrap_or(usize::MAX),
            });
        }
        // The fixed upper bound also covers a file that grew after metadata.
        let mut bytes = Vec::with_capacity(33);
        file.take(33)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error(&path, error))?;
        let length = bytes.len();
        let key = bytes
            .try_into()
            .map_err(|_| KeyError::Malformed { path, length })?;
        Ok(WorkspaceKey(key))
    }

    fn fresh_file(&self) -> Result<(WorkspaceKey, tempfile::NamedTempFile), KeyError> {
        let directory = self.state_dir.join("keys");
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|error| KeyError::Entropy(error.to_string()))?;
        let mut file = tempfile::NamedTempFile::new_in(&directory)
            .map_err(|error| io_error(&directory, error))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|error| io_error(file.path(), error))?;
        }
        file.write_all(&bytes)
            .map_err(|error| io_error(file.path(), error))?;
        file.as_file()
            .sync_all()
            .map_err(|error| io_error(file.path(), error))?;
        Ok((WorkspaceKey(bytes), file))
    }
}

fn ensure_directory(path: &Path) -> Result<(), KeyError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(KeyError::UnsafeLocation(path.to_path_buf()));
        }
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(path, error)),
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|error| io_error(path, error))?;
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(KeyError::UnsafeLocation(path.to_path_buf()));
    }
    Ok(())
}

fn check_regular_target(path: &Path) -> Result<(), KeyError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err(KeyError::UnsafeLocation(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(path, error)),
    }
}

fn io_error(path: &Path, source: std::io::Error) -> KeyError {
    KeyError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
#[path = "workspace_key_store.tests.rs"]
mod tests;
