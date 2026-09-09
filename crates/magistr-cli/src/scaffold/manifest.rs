//! Scaffold manifest — tracks content hashes of magistr-generated files.
//!
//! The manifest (`.magistr/scaffold-manifest.json`) records a hash of each file at
//! the time magistr wrote it. During re-init, this lets us detect whether the user
//! has customized a file (disk hash differs from manifest hash) so we can
//! preserve their edits instead of blindly overwriting.

use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};

// Path now derived from `crate::paths::scaffold_manifest()` — no local constant needed.

/// Scaffold manifest: maps relative file paths to their content hashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScaffoldManifest {
    /// File path (relative to project root) → hex-encoded content hash.
    pub(crate) files: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) codex: Option<super::codex_ownership::CodexOwnership>,
}

impl ScaffoldManifest {
    /// Create an empty manifest.
    pub(crate) fn new() -> Self {
        Self {
            files: BTreeMap::new(),
            codex: None,
        }
    }

    /// Load the manifest from disk. Returns an empty manifest if the file
    /// doesn't exist or can't be parsed (graceful degradation).
    pub(crate) fn load(dir: &Path) -> Self {
        let path = crate::paths::scaffold_manifest(dir);
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| Self::new()),
            Err(_) => Self::new(),
        }
    }

    /// Save the manifest to disk.
    pub(crate) fn save(&self, dir: &Path) -> Result<()> {
        let path = crate::paths::scaffold_manifest(dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content).map_err(|e| MagistrError::io(&path, e))?;
        Ok(())
    }

    /// Record a file's content hash in the manifest.
    pub(crate) fn record(&mut self, path: &str, content: &str) {
        self.files.insert(path.to_string(), content_hash(content));
    }

    /// Check if a file on disk has been customized by the user.
    ///
    /// Returns `true` if the file's current disk content differs from the hash
    /// recorded in the manifest (meaning the user edited it after magistr wrote it).
    ///
    /// Returns `true` (treat as customized) if there's no manifest entry for the
    /// file — this handles files that predate the manifest system.
    pub(crate) fn is_customized(&self, path: &str, disk_content: &str) -> bool {
        match self.files.get(path) {
            Some(recorded_hash) => *recorded_hash != content_hash(disk_content),
            None => true, // No record → assume user-created, don't overwrite
        }
    }
}

/// Compute a hex-encoded hash of the given content.
///
/// Uses `DefaultHasher` (SipHash) — not cryptographic, but sufficient for
/// detecting content changes in scaffold files.
pub(crate) fn content_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
#[path = "manifest.tests.rs"]
mod tests;
