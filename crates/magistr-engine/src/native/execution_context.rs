use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};

/// A connection-bound execution selection within a canonical working copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub working_copy: PathBuf,
    pub execution_id: String,
}

impl ExecutionContext {
    pub fn new(root: &Path, execution_id: impl Into<String>) -> io::Result<Self> {
        let working_copy = root.canonicalize()?;
        let execution_id = execution_id.into();
        if execution_id.trim().is_empty() || execution_id.len() > 256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The work identity is invalid.",
            ));
        }
        Ok(Self {
            working_copy,
            execution_id,
        })
    }

    /// A safe persistence key; authored names and paths never become filenames.
    #[must_use]
    pub fn key(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(self.working_copy.as_os_str().as_encoded_bytes());
        hash.update([0]);
        hash.update(self.execution_id.as_bytes());
        hex::encode(hash.finalize())
    }
}

#[cfg(test)]
#[path = "execution_context.tests.rs"]
mod tests;
