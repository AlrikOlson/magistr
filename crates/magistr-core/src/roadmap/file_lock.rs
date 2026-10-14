//! File-based advisory locking for roadmap YAML.
//!
//! Uses `flock(2)` on Unix to serialize concurrent access to
//! `.magistr-roadmap.yml` from MCP tools and the interface.

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};

/// An advisory file lock backed by `flock(2)`.
///
/// The lock is released when this guard is dropped.
pub struct FileLock {
    _file: File,
    lock_path: PathBuf,
}

impl FileLock {
    /// Acquire an exclusive lock for the given YAML path.
    ///
    /// Creates a `.lock` file alongside the target and holds `LOCK_EX`
    /// until this guard is dropped. Blocks if another process holds the lock.
    pub fn exclusive(yaml_path: &Path) -> Result<Self> {
        let lock_path = lock_path_for(yaml_path);
        let file = open_lock_file(&lock_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let ret = unsafe { libc::flock(fd, libc::LOCK_EX) };
            if ret != 0 {
                return Err(MagistrError::Config(format!(
                    "failed to acquire exclusive lock on {}: {}",
                    lock_path.display(),
                    std::io::Error::last_os_error(),
                )));
            }
        }

        Ok(Self {
            _file: file,
            lock_path,
        })
    }

    /// Acquire a shared (read) lock for the given YAML path.
    ///
    /// Multiple readers can hold shared locks concurrently, but an exclusive
    /// lock blocks until all shared locks are released.
    pub fn shared(yaml_path: &Path) -> Result<Self> {
        let lock_path = lock_path_for(yaml_path);
        let file = open_lock_file(&lock_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            let ret = unsafe { libc::flock(fd, libc::LOCK_SH) };
            if ret != 0 {
                return Err(MagistrError::Config(format!(
                    "failed to acquire shared lock on {}: {}",
                    lock_path.display(),
                    std::io::Error::last_os_error(),
                )));
            }
        }

        Ok(Self {
            _file: file,
            lock_path,
        })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // flock is released automatically when the fd is closed.
        // The lock file is intentionally NOT deleted — it's harmless and
        // avoids a TOCTOU race where another process tries to flock a
        // file that was just deleted.
        let _ = &self.lock_path; // suppress unused field warning
    }
}

/// Derive the lock file path from the YAML path.
fn lock_path_for(yaml_path: &Path) -> PathBuf {
    let mut lock = yaml_path.as_os_str().to_os_string();
    lock.push(".lock");
    PathBuf::from(lock)
}

/// Open (or create) the lock file.
fn open_lock_file(lock_path: &Path) -> Result<File> {
    // Ensure parent directory exists.
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            MagistrError::Config(format!(
                "failed to create directory for lock file {}: {e}",
                lock_path.display()
            ))
        })?;
    }

    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| {
            MagistrError::Config(format!(
                "failed to open lock file {}: {e}",
                lock_path.display()
            ))
        })
}

#[cfg(test)]
#[path = "file_lock.tests.rs"]
mod tests;
