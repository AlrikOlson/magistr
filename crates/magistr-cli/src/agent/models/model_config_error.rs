//! Errors from model config operations.

use std::path::PathBuf;

/// Errors from model config operations.
#[derive(Debug)]
pub enum ModelConfigError {
    /// File I/O error.
    Io(PathBuf, std::io::Error),
    /// TOML parse error.
    Parse(PathBuf, String),
    /// TOML serialization error.
    Serialize(String),
}

impl std::fmt::Display for ModelConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "model config I/O error at '{}': {e}", path.display()),
            Self::Parse(path, e) => {
                write!(f, "failed to parse models config '{}': {e}", path.display())
            }
            Self::Serialize(e) => write!(f, "failed to serialize models config: {e}"),
        }
    }
}

impl std::error::Error for ModelConfigError {}
