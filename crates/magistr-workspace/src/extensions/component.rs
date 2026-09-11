use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::workspace_policy::WorkspacePolicy;

/// Ceiling on component bytes the host will hand to the worker.
const COMPONENT_MAX_BYTES: u64 = 24 * 1024 * 1024;

/// A plugin component resolved through the workspace jail and hashed: the
/// hash is the breaker key, the bytes travel to the worker per invoke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginComponent {
    pub abs_path: PathBuf,
    pub sha256: String,
    pub bytes: Vec<u8>,
}

impl PluginComponent {
    /// Resolve `dir/component_path` inside the workspace; `None` when the file
    /// is missing, escapes the jail, is not a regular file or is oversized.
    #[must_use]
    pub fn resolve(policy: &WorkspacePolicy, dir: &str, component_path: &str) -> Option<Self> {
        let relative = format!("{}/{}", dir.trim_end_matches('/'), component_path);
        let admitted = policy.check_read(&relative).ok()?;
        Self::read(&admitted)
    }

    /// Read one component from an absolute path outside any jail — the TCK
    /// runs against a directory on the operator's machine.
    #[must_use]
    pub fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() || metadata.len() > COMPONENT_MAX_BYTES {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        Some(Self {
            abs_path: path.to_path_buf(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
        })
    }
}
