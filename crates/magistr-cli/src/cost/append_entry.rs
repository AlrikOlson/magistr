//! Append a cost entry to the history file.

use std::path::Path;

use crate::error::{MagistrError, Result};

use super::cost_entry::CostEntry;
use super::storage::load_entries;

/// Append a single cost entry to the file.
///
/// Creates the file and parent directories if they don't exist.
pub fn append_entry(path: &Path, entry: &CostEntry) -> Result<()> {
    let mut entries = load_entries(path)?;
    entries.push(entry.clone());
    save_entries(path, &entries)
}

/// Save a full list of entries, replacing the file contents.
fn save_entries(path: &Path, entries: &[CostEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
    }

    let json = serde_json::to_string_pretty(entries)?;

    std::fs::write(path, json).map_err(|e| MagistrError::io(path, e))
}
