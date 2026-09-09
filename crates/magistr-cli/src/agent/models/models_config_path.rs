//! Resolve the models config file path.

use std::path::PathBuf;

/// Resolve the models config file path.
///
/// Uses `$XDG_CONFIG_HOME/magistr/models.toml` if set,
/// otherwise `~/.config/magistr/models.toml`.
pub fn models_config_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("magistr/models.toml"));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/magistr/models.toml"))
}
