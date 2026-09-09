//! User-defined preset loading from `~/.config/magistr/presets/`.
//!
//! Users can place preset files (TOML or JSON) in their config directory
//! to override built-in defaults. Files are named by language slug
//! (e.g., `rust.toml`, `rust.json`) and use the `PresetOverride`
//! schema so only specified fields are changed.

use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};
use crate::types::{Language, PresetOverride};

use super::preset_fragment::UserPresetFragment;
use super::project_preset::{PresetInfo, list_presets_in_dir};

/// Default user preset directory following XDG conventions.
const USER_PRESET_DIR: &str = ".config/magistr/presets";

/// Resolve the user preset directory path.
///
/// Uses `$XDG_CONFIG_HOME/magistr/presets` if set, otherwise `~/.config/magistr/presets`.
pub fn user_preset_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg).join("magistr/presets");
        return Some(path);
    }

    dirs_path().map(|home| home.join(USER_PRESET_DIR))
}

/// Get the home directory path.
fn dirs_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Try to load a user-defined preset override for the given language.
///
/// Checks for TOML first, then JSON. Returns `None` if no user preset exists.
/// Returns `Err` if the file exists but is unparseable.
pub fn load_user_override(lang: Language) -> Result<Option<PresetOverride>> {
    let dir = match user_preset_dir() {
        Some(d) => d,
        None => return Ok(None),
    };

    load_user_override_from_dir(&dir, lang)
}

/// Load a user override from a specific directory (for testing).
pub fn load_user_override_from_dir(dir: &Path, lang: Language) -> Result<Option<PresetOverride>> {
    // Try TOML first (new format), then JSON (legacy)
    let toml_path = dir.join(format!("{}.toml", lang.as_str()));
    if toml_path.is_file() {
        let content =
            std::fs::read_to_string(&toml_path).map_err(|e| MagistrError::io(&toml_path, e))?;
        let frag = UserPresetFragment::from_toml(&content).map_err(|e| {
            MagistrError::Config(format!(
                "invalid user preset at {}: {e}",
                toml_path.display()
            ))
        })?;
        return Ok(Some(frag.to_override()));
    }

    let json_path = dir.join(format!("{}.json", lang.as_str()));
    if json_path.is_file() {
        let content =
            std::fs::read_to_string(&json_path).map_err(|e| MagistrError::io(&json_path, e))?;
        let override_preset: PresetOverride = serde_json::from_str(&content).map_err(|e| {
            MagistrError::Config(format!(
                "invalid user preset at {}: {e}",
                json_path.display()
            ))
        })?;
        return Ok(Some(override_preset));
    }

    Ok(None)
}

/// List all user preset files that exist in the preset directory.
pub fn list_user_presets() -> Vec<PathBuf> {
    let dir = match user_preset_dir() {
        Some(d) if d.is_dir() => d,
        _ => return vec![],
    };

    list_user_presets_in_dir(&dir)
}

/// List user preset files in a specific directory (for testing).
fn list_user_presets_in_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };

    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext == "json" || ext == "toml")
        })
        .collect()
}

/// List user presets with metadata for the `preset list` command.
pub fn list_user_preset_info() -> Vec<PresetInfo> {
    let dir = match user_preset_dir() {
        Some(d) if d.is_dir() => d,
        _ => return vec![],
    };

    list_presets_in_dir(&dir, "user-global")
}

#[cfg(test)]
#[path = "user_preset.tests.rs"]
mod tests;
