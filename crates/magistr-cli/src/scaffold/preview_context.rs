use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use crate::error::Result;

/// The external evidence that makes a Codex scaffold preview safe to apply.
pub(super) struct PreviewContext;

impl PreviewContext {
    pub(super) fn capture(dir: &Path) -> Result<String> {
        let mut hash = DefaultHasher::new();
        for path in [".magistr/scaffold-manifest.json", ".magistr/project.json"] {
            // Hashing Option<String> distinguishes absence from an empty file;
            // hashing each path keeps the two snapshots separately bounded.
            path.hash(&mut hash);
            super::CodexSetup::read(dir, path)?.hash(&mut hash);
        }
        Ok(format!("{:016x}", hash.finish()))
    }
}
