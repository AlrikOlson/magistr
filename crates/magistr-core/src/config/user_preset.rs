//! User-defined preset loading from the captured user preset directory
//! (`$XDG_CONFIG_HOME/magistr/presets` or `~/.config/magistr/presets`, as
//! [`ProjectInputs`](crate::config::inputs::ProjectInputs) resolves it).
//!
//! Users can place preset files (TOML or JSON) in that directory to
//! override built-in defaults. Files are named by language slug
//! (e.g., `rust.toml`, `rust.json`) and use the `PresetOverride`
//! schema so only specified fields are changed.

use std::path::Path;

use crate::error::{MagistrError, Result};
use crate::types::{Language, PresetOverride};

use super::preset_fragment::UserPresetFragment;

/// Try to load a user-defined preset override for `lang` from `dir`.
///
/// Checks for TOML first, then JSON. Returns `None` if no user preset exists.
/// Returns `Err` if the file exists but is unparseable.
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

#[cfg(test)]
#[path = "user_preset.tests.rs"]
mod tests;
