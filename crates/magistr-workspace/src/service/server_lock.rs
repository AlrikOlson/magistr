use std::{
    fs::{File, OpenOptions, TryLockError},
    io,
    path::{Path, PathBuf},
};

/// One admitted workspace process: an OS-held exclusive lock on
/// `<state>/server.lock`. The kernel owns the decision — no pid is read, no
/// age is judged, nothing is stolen and no holder is ever signalled — so a
/// crashed owner releases on exit and a live one is refused whatever its
/// lock file says. This is single-owner admission for one process; the
/// multi-client owner protocol layers on top of it later.
#[derive(Debug)]
pub struct ServerLock {
    file: File,
    path: PathBuf,
}

impl ServerLock {
    pub const FILE_NAME: &'static str = "server.lock";

    /// Take the lock, or answer the live holder's refusal as `WouldBlock`.
    /// A symlink or a non-regular file at the path is refused outright:
    /// the lock is never taken through an alias.
    pub fn acquire(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() || !metadata.is_file() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{} is not a regular lock file", path.display()),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        match file.try_lock() {
            Ok(()) => Ok(Self { file, path }),
            Err(TryLockError::WouldBlock) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "another workspace server already owns this workspace (lock: {})",
                    path.display()
                ),
            )),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        // A concurrently spawned child can inherit this descriptor until it
        // execs; closing ours alone would leave the lock held through that
        // reference. Explicit unlock ends ownership when the owner closes.
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
#[path = "server_lock.tests.rs"]
mod tests;
