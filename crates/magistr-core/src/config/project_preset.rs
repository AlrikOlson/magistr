//! Project-local preset loading from `.magistr/presets/`.
//!
//! Users can place preset files (TOML or JSON) in their project's
//! `.magistr/presets/` directory. These are merged as `ProjectLocal`
//! layers, after built-in layers but before user-global overrides.
//!
//! TOML files are parsed as `UserPresetFragment` (the new schema with
//! metadata). JSON files are parsed as either `UserPresetFragment` or
//! plain `PresetOverride` (backward-compatible).

use std::path::Path;

use crate::error::{MagistrError, Result};
use crate::types::{PresetLayer, PresetLayerType, PresetOverride};

use super::preset_fragment::UserPresetFragment;

/// Subdirectory within the project root for local preset fragments.
pub const PROJECT_PRESET_DIR: &str = ".magistr/presets";

/// Load all project-local preset fragments from `<project_root>/.magistr/presets/`.
///
/// Returns an empty vec if the directory doesn't exist (graceful fallback).
/// Returns `Err` if a file exists but is unparseable.
pub fn load_project_presets(project_root: &Path) -> Result<Vec<PresetLayer>> {
    let dir = project_root.join(PROJECT_PRESET_DIR);
    load_project_presets_from_dir(&dir)
}

/// Load project-local presets from a specific directory (for testing).
pub fn load_project_presets_from_dir(dir: &Path) -> Result<Vec<PresetLayer>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| MagistrError::io(dir, e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext == "json" || ext == "toml")
        })
        .collect();

    // Sort for deterministic ordering within the ProjectLocal layer type.
    entries.sort();

    let mut layers = Vec::new();
    for path in entries {
        let layer = load_single_preset(&path)?;
        layers.push(layer);
    }

    Ok(layers)
}

/// Load a single preset file (TOML or JSON) as a `PresetLayer`.
fn load_single_preset(path: &Path) -> Result<PresetLayer> {
    let is_toml = path.extension().is_some_and(|ext| ext == "toml");
    let content = std::fs::read_to_string(path).map_err(|e| MagistrError::io(path, e))?;

    let (name, overrides) = if is_toml {
        let frag = UserPresetFragment::from_toml(&content).map_err(|e| {
            MagistrError::Config(format!("invalid project preset at {}: {e}", path.display()))
        })?;
        (frag.name.clone(), frag.to_override())
    } else {
        // Try new fragment schema first, fall back to plain PresetOverride
        match UserPresetFragment::from_json(&content) {
            Ok(frag) => (frag.name.clone(), frag.to_override()),
            Err(_) => {
                let overrides: PresetOverride = serde_json::from_str(&content).map_err(|e| {
                    MagistrError::Config(format!(
                        "invalid project preset at {}: {e}",
                        path.display()
                    ))
                })?;
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                (name, overrides)
            }
        }
    };

    Ok(PresetLayer {
        layer_type: PresetLayerType::ProjectLocal,
        name,
        overrides,
    })
}

/// List all preset files in a directory with their metadata.
///
/// Returns `(path, name, source)` tuples for display in `preset list`.
pub fn list_project_presets(project_root: &Path) -> Vec<PresetInfo> {
    let dir = project_root.join(PROJECT_PRESET_DIR);
    list_presets_in_dir(&dir, "project-local")
}

/// Info about a discovered preset for display purposes.
#[derive(Debug, Clone)]
pub struct PresetInfo {
    pub name: String,
    pub source: String,
    pub layer_type: String,
    pub path: std::path::PathBuf,
    pub description: Option<String>,
    pub extends: Option<String>,
}

/// List preset files in a directory with metadata.
pub fn list_presets_in_dir(dir: &Path, source: &str) -> Vec<PresetInfo> {
    if !dir.is_dir() {
        return vec![];
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext == "json" || ext == "toml")
        })
        .collect();
    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            let is_toml = path.extension().is_some_and(|ext| ext == "toml");

            if is_toml {
                let frag = UserPresetFragment::from_toml(&content).ok()?;
                Some(PresetInfo {
                    name: frag.name,
                    source: source.into(),
                    layer_type: format!("{:?}", frag.layer_type).to_lowercase(),
                    path,
                    description: frag.description,
                    extends: frag.extends,
                })
            } else {
                // Try fragment schema, fall back to generic name
                match UserPresetFragment::from_json(&content) {
                    Ok(frag) => Some(PresetInfo {
                        name: frag.name,
                        source: source.into(),
                        layer_type: format!("{:?}", frag.layer_type).to_lowercase(),
                        path,
                        description: frag.description,
                        extends: frag.extends,
                    }),
                    Err(_) => {
                        let name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();
                        Some(PresetInfo {
                            name,
                            source: source.into(),
                            layer_type: "project-local".into(),
                            path,
                            description: None,
                            extends: None,
                        })
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "project_preset.tests.rs"]
mod tests;
