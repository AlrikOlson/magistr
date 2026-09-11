use std::path::PathBuf;

/// A key failure never causes silent regeneration of existing material.
#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("workspace key I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("workspace key at {path} is {length} bytes, expected 32; explicitly rotate it")]
    Malformed { path: PathBuf, length: usize },
    #[error("workspace key location is not a regular file or directory: {0}")]
    UnsafeLocation(PathBuf),
    #[error("workspace key entropy failed: {0}")]
    Entropy(String),
}
