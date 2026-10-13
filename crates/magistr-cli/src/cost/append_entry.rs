//! Store the latest cumulative spend for each logical conversation in the ledger.

use std::path::Path;

use crate::error::{MagistrError, Result};

use super::cost_entry::CostEntry;
use super::storage::load_entries;

/// Replace this conversation's cumulative entry, or append its first entry.
///
/// Creates the file and parent directories if they don't exist.
pub fn append_entry(path: &Path, entry: &CostEntry) -> Result<()> {
    let mut entries = load_entries(path)?;
    if let Some(previous) = entries
        .iter_mut()
        .find(|previous| previous.session_id == entry.session_id)
    {
        *previous = entry.clone();
    } else {
        entries.push(entry.clone());
    }
    save_entries(path, &entries)
}

/// Save a full list of entries, replacing the file contents.
fn save_entries(path: &Path, entries: &[CostEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
    }

    let json = serde_json::to_string_pretty(entries)?;

    // Whole-file replacement, atomically: a crash mid-write cannot leave a
    // torn ledger that every later session would fail to read.
    crate::workflow::durable_write::durable_write(path, json.as_bytes())
        .map_err(|e| MagistrError::io(path, e))
}
