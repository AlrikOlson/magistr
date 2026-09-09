//! Load cost entries from JSON file storage.

use std::path::Path;

use crate::error::{MagistrError, Result};

use super::cost_entry::CostEntry;

/// Load all cost entries from a file.
///
/// Returns an empty vec if the file does not exist.
/// Returns an error if the file exists but is invalid JSON.
pub fn load_entries(path: &Path) -> Result<Vec<CostEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(path).map_err(|e| MagistrError::io(path, e))?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&content)
        .map_err(|e| MagistrError::Config(format!("invalid costs file: {e}")))
}

#[cfg(test)]
#[path = "storage.tests.rs"]
mod tests;
