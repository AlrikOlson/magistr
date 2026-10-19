use std::path::PathBuf;

use super::JournalMode;

/// Native writer options for the retained `.magistr/workspace/journal.db` format.
#[derive(Clone, Debug)]
pub struct JournalDbOptions {
    pub path: PathBuf,
    pub journal_mode: JournalMode,
    /// Compressed-byte retention cap; an append always retains its own blobs.
    pub cap_bytes: u64,
}

impl JournalDbOptions {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            journal_mode: JournalMode::Wal,
            cap_bytes: 256 * 1024 * 1024,
        }
    }
}
