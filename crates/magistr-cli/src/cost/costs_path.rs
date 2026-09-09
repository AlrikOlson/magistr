//! Resolve the full path to the costs file.

use std::path::PathBuf;

use super::storage_paths::data_dir;

/// Default filename for cost history.
const COSTS_FILENAME: &str = "costs.json";

/// Resolve the full path to the costs file.
pub fn costs_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join(COSTS_FILENAME))
}
