use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ChangeKind, ManifestEntry, SourceChange, workspace_identity::frame};

/// Every selected path with what was observed there, in path order.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceManifest {
    pub entries: BTreeMap<String, ManifestEntry>,
}

impl SourceManifest {
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// SHA-256 over every entry's path, kind, mode, size and digest, each
    /// length-framed, in path order. Content only: no clock, no inode.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut digest = Sha256::new();
        frame(&mut digest, &(self.entries.len() as u64).to_le_bytes());
        for (path, entry) in &self.entries {
            frame(&mut digest, path.as_bytes());
            frame(&mut digest, entry.kind.as_str().as_bytes());
            frame(&mut digest, &entry.mode.to_le_bytes());
            frame(&mut digest, &entry.size.to_le_bytes());
            frame(&mut digest, entry.digest.as_bytes());
        }
        format!("{:x}", digest.finalize())
    }

    /// The paths that differ between this manifest and a later observation.
    #[must_use]
    pub fn changes_to(&self, later: &Self) -> Vec<SourceChange> {
        let mut changes = Vec::new();
        for (path, entry) in &self.entries {
            match later.entries.get(path) {
                None => changes.push(SourceChange {
                    path: path.clone(),
                    kind: ChangeKind::Removed,
                }),
                Some(observed) if observed != entry => changes.push(SourceChange {
                    path: path.clone(),
                    kind: ChangeKind::Modified,
                }),
                Some(_) => {}
            }
        }
        for path in later.entries.keys() {
            if !self.entries.contains_key(path) {
                changes.push(SourceChange {
                    path: path.clone(),
                    kind: ChangeKind::Added,
                });
            }
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        changes
    }
}
