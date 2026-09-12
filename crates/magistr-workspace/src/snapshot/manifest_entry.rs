use serde::{Deserialize, Serialize};

use super::EntryKind;

/// One observed path: its kind, permission bits, byte length and the SHA-256
/// of its bytes (a symlink's target for symlinks). No mtime, inode or device
/// number is recorded, so none of them can ever influence a revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub kind: EntryKind,
    /// Unix permission bits (`mode & 0o7777`); on other platforms `0o444`
    /// for read-only and `0o644` otherwise.
    pub mode: u32,
    pub size: u64,
    /// Lowercase hex SHA-256; empty for a missing entry.
    pub digest: String,
}

impl ManifestEntry {
    /// The record for an explicit input that does not exist.
    #[must_use]
    pub const fn missing() -> Self {
        Self {
            kind: EntryKind::Missing,
            mode: 0,
            size: 0,
            digest: String::new(),
        }
    }
}
