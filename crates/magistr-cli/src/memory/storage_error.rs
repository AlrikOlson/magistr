//! Errors from memory storage operations.

use std::path::PathBuf;

/// Errors from memory storage operations.
#[derive(Debug)]
pub enum MemoryStorageError {
    /// File I/O error.
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for MemoryStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "memory I/O error at '{}': {}", path.display(), e),
        }
    }
}

impl std::error::Error for MemoryStorageError {}
