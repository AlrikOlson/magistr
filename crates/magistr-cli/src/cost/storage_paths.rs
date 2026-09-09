//! Resolve data directory and costs file path.

use std::path::PathBuf;

/// Resolve the data directory for magistr.
///
/// Uses `$XDG_DATA_HOME/magistr/` if set, otherwise `~/.local/share/magistr/`.
pub fn data_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return Some(PathBuf::from(xdg).join("magistr"));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/magistr"))
}
