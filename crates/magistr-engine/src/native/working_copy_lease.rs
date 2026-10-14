use std::{fs::File, io, path::Path};

/// Shared while work is open; exclusive while a working copy is archived or reopened.
#[derive(Debug)]
pub struct WorkingCopyLease {
    _file: File,
}

impl WorkingCopyLease {
    pub fn active(root: &Path) -> io::Result<Self> {
        Self::acquire(root, false)
    }
    pub fn archive(root: &Path) -> io::Result<Self> {
        Self::acquire(root, true)
    }

    fn acquire(root: &Path, exclusive: bool) -> io::Result<Self> {
        let root = super::ProjectLocation::working_copy_root(root)?;
        let directory = root.join(".magistr");
        std::fs::create_dir_all(&directory)?;
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("working-copy.lock"))?;
        let result = if exclusive {
            file.try_lock()
        } else {
            file.try_lock_shared()
        };
        result.map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                "Work is still open in this working copy. Close it before archiving.",
            ),
            std::fs::TryLockError::Error(error) => error,
        })?;
        Ok(Self { _file: file })
    }
}
#[cfg(test)]
#[path = "working_copy_lease.tests.rs"]
mod tests;
