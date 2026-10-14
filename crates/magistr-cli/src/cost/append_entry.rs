//! Store the latest cumulative spend for each logical conversation in the ledger.

use std::fs::File;
use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};

use super::cost_entry::CostEntry;
use super::storage::load_entries;

/// Replace this conversation's cumulative entry, or append its first entry.
///
/// Creates the file and parent directories if they don't exist. Writers in
/// every process hold the same sibling lock through the full read and replace.
pub fn append_entry(path: &Path, entry: &CostEntry) -> Result<()> {
    let _lock = lock_ledger(path)?;
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

/// The lock file stays in place: locking the ledger itself would lose mutual
/// exclusion when the atomic replacement changes its inode.
fn lock_ledger(path: &Path) -> Result<File> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| MagistrError::io(parent, error))?;
    let mut name = path.as_os_str().to_os_string();
    name.push(".lock");
    let lock_path = PathBuf::from(name);
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| MagistrError::io(&lock_path, error))?;
    lock.lock()
        .map_err(|error| MagistrError::io(&lock_path, error))?;
    Ok(lock)
}

/// Save a full list of entries, replacing the file contents.
fn save_entries(path: &Path, entries: &[CostEntry]) -> Result<()> {
    let json = serde_json::to_string_pretty(entries)?;

    // Whole-file replacement, atomically: a crash mid-write cannot leave a
    // torn ledger that every later session would fail to read.
    magistr_core::durable_write::durable_write(path, json.as_bytes())
        .map_err(|e| MagistrError::io(path, e))
}

#[cfg(test)]
#[path = "append_entry.tests.rs"]
mod tests;
