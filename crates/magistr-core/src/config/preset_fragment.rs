//! User-authored preset fragments (`.magistr/presets/` or `~/.config/magistr/presets/`).
//!
//! A preset fragment is a partial preset override with metadata:
//! name, description, layer type, optional inheritance via `extends`,
//! and merge strategy. Supports both TOML (recommended for authoring)
//! and JSON (backward-compatible).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};
use crate::types::{
    FormatConfigOverride, PresetLayerType, PresetOverride, QualityGateOverride,
    TestingConfigOverride,
};

/// How list-valued fields (e.g., `extra_allowed_tools`) are merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    /// Replace the parent value entirely (default for scalar fields).
    #[default]
    Replace,
    /// Append to the parent list (useful for `extra_allowed_tools`).
    Append,
}

/// A user-authored preset fragment with metadata.
///
/// This is the on-disk schema for files in `.magistr/presets/` (project-local)
/// and `~/.config/magistr/presets/` (user-global). It wraps `PresetOverride`
/// with additional metadata for the composition pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserPresetFragment {
    /// Human-readable name (e.g., "my-team-rust").
    pub name: String,

    /// Optional description of what this fragment configures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Layer type in the composition pipeline.
    /// Defaults to `project-local` for `.magistr/presets/` files.
    #[serde(default = "default_layer_type")]
    pub layer_type: FragmentLayerType,

    /// Inherit from a built-in language preset by name (e.g., "rust", "python").
    /// When set, the fragment's overrides are applied on top of the named preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,

    /// How list fields are merged with parent layers.
    #[serde(default)]
    pub merge_strategy: MergeStrategy,

    /// Quality gate command overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_gate: Option<QualityGateOverride>,

    /// Formatter configuration overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatConfigOverride>,

    /// Testing configuration overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub testing: Option<TestingConfigOverride>,

    /// Additional allowed tools to add to the preset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_allowed_tools: Vec<String>,
}

/// Wrapper for `PresetLayerType` that provides serde with string names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FragmentLayerType {
    ProjectLocal,
    UserOverride,
}

impl From<FragmentLayerType> for PresetLayerType {
    fn from(f: FragmentLayerType) -> Self {
        match f {
            FragmentLayerType::ProjectLocal => PresetLayerType::ProjectLocal,
            FragmentLayerType::UserOverride => PresetLayerType::UserOverride,
        }
    }
}

fn default_layer_type() -> FragmentLayerType {
    FragmentLayerType::ProjectLocal
}

/// A validation issue found in a preset fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentWarning {
    pub field: String,
    pub message: String,
    pub is_error: bool,
}

impl FragmentWarning {
    fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            is_error: true,
        }
    }

    fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            is_error: false,
        }
    }
}

impl UserPresetFragment {
    /// Parse a fragment from TOML.
    pub fn from_toml(s: &str) -> Result<Self> {
        toml::from_str(s).map_err(|e| MagistrError::Config(format!("TOML parse: {e}")))
    }

    /// Parse a fragment from JSON.
    pub fn from_json(s: &str) -> Result<Self> {
        serde_json::from_str(s).map_err(|e| MagistrError::Config(format!("JSON parse: {e}")))
    }

    /// Serialize to TOML.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| MagistrError::Config(format!("TOML serialize: {e}")))
    }

    /// Load a fragment from a file, detecting format by extension.
    ///
    /// `.toml` files are parsed as TOML, everything else as JSON.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| MagistrError::io(path, e))?;
        let is_toml = path.extension().is_some_and(|ext| ext == "toml");
        if is_toml {
            Self::from_toml(&content)
        } else {
            Self::from_json(&content)
        }
    }

    /// Save this fragment to a TOML file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = self.to_toml()?;
        std::fs::write(path, content).map_err(|e| MagistrError::io(path, e))
    }

    /// Convert to a `PresetOverride` for use in the composition pipeline.
    pub fn to_override(&self) -> PresetOverride {
        PresetOverride {
            quality_gate: self.quality_gate.clone(),
            format: self.format.clone(),
            testing: self.testing.clone(),
            extra_allowed_tools: self.extra_allowed_tools.clone(),
        }
    }

    /// Validate the fragment and return any issues.
    pub fn validate(&self) -> Vec<FragmentWarning> {
        let mut warnings = Vec::new();

        if self.name.trim().is_empty() {
            warnings.push(FragmentWarning::error("name", "preset name is required"));
        }

        if let Some(extends) = &self.extends
            && extends.trim().is_empty()
        {
            warnings.push(FragmentWarning::error(
                "extends",
                "extends field is set but empty",
            ));
        }

        // Warn if fragment has no actual overrides
        if self.quality_gate.is_none()
            && self.format.is_none()
            && self.testing.is_none()
            && self.extra_allowed_tools.is_empty()
        {
            warnings.push(FragmentWarning::warning(
                "(root)",
                "fragment has no overrides — it won't change anything",
            ));
        }

        // Validate gate commands are non-empty when present
        if let Some(qg) = &self.quality_gate {
            if qg.check.as_deref().is_some_and(|s| s.trim().is_empty()) {
                warnings.push(FragmentWarning::warning(
                    "quality_gate.check",
                    "check command is empty string — will clear the gate",
                ));
            }
            if qg.test.as_deref().is_some_and(|s| s.trim().is_empty()) {
                warnings.push(FragmentWarning::warning(
                    "quality_gate.test",
                    "test command is empty string — will clear the gate",
                ));
            }
            if qg.lint.as_deref().is_some_and(|s| s.trim().is_empty()) {
                warnings.push(FragmentWarning::warning(
                    "quality_gate.lint",
                    "lint command is empty string — will clear the gate",
                ));
            }
        }

        warnings
    }

    /// Returns true if validation found any error-severity issues.
    pub fn has_errors(&self) -> bool {
        self.validate().iter().any(|w| w.is_error)
    }

    /// Resolve the `extends` field by loading the named built-in preset
    /// and returning the fully resolved `Preset`.
    ///
    /// If `extends` is `None`, returns `None` — the fragment is used as
    /// a plain override layer in the pipeline.
    pub fn resolve_inheritance(&self) -> Result<Option<crate::types::Preset>> {
        let Some(base_name) = &self.extends else {
            return Ok(None);
        };

        let lang = crate::types::Language::from_str_loose(base_name).ok_or_else(|| {
            MagistrError::Config(format!(
                "extends: unknown preset '{base_name}' — must be a built-in language name"
            ))
        })?;

        let base = super::preset::load_preset(lang)?;
        let resolved = base.merge(&self.to_override());
        Ok(Some(resolved))
    }
}

#[cfg(test)]
#[path = "preset_fragment.tests.rs"]
mod tests;
