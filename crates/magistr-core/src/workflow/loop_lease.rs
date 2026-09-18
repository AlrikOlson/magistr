//! Mutual exclusion between a running loop and explicit checkpoint resolution.

use std::fs::{File, TryLockError};
use std::path::Path;

use crate::error::{MagistrError, Result};

/// Held for a loop command's entire lifetime, or a single operator resolution.
/// The stable lock file is never removed; the OS releases its lock on exit.
pub struct LoopLease {
    _file: File,
}

impl LoopLease {
    pub fn acquire(project_dir: &Path) -> Result<Self> {
        let parent = project_dir.join(".magistr");
        std::fs::create_dir_all(&parent).map_err(|error| MagistrError::io(&parent, error))?;
        let path = parent.join("loop-owner.lock");
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| MagistrError::io(&path, error))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(MagistrError::Workflow(
                "A loop is already running or its saved progress is being updated. Try again after it stops.".into(),
            )),
            Err(TryLockError::Error(error)) => Err(MagistrError::io(&path, error)),
        }
    }
}

#[cfg(test)]
#[path = "loop_lease.tests.rs"]
mod tests;
