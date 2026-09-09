//! Layered preset composition: language → framework → package-manager →
//! database → CI/CD → deployment → project-local → user overrides.

use super::Preset;
use super::preset::PresetOverride;

/// The type of a preset layer, ordered by composition priority.
///
/// Layers are applied in ascending order — later layers override earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PresetLayerType {
    /// Base language preset (e.g., `rust.json`).
    Language,
    /// Framework overlay (e.g., `axum.json`).
    Framework,
    /// Package-manager overlay (e.g., `gradle.json`).
    PackageManager,
    /// Database/ORM overlay (e.g., `postgresql-sqlx.json`).
    Database,
    /// CI/CD provider overlay (e.g., `github-actions.json`).
    CiCd,
    /// Deployment target overlay (e.g., `docker.json`).
    Deployment,
    /// Project-local preset from `.magistr/presets/`.
    ProjectLocal,
    /// User-global override from `~/.config/magistr/presets/`.
    UserOverride,
}

impl PresetLayerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Language => "language",
            Self::Framework => "framework",
            Self::PackageManager => "package-manager",
            Self::Database => "database",
            Self::CiCd => "ci-cd",
            Self::Deployment => "deployment",
            Self::ProjectLocal => "project-local",
            Self::UserOverride => "user-override",
        }
    }
}

impl std::fmt::Display for PresetLayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single preset layer that contributes overrides to the composition pipeline.
#[derive(Debug, Clone)]
pub struct PresetLayer {
    /// What kind of layer this is (determines application order).
    pub layer_type: PresetLayerType,
    /// Human-readable name for logging/debugging.
    pub name: String,
    /// The override to apply.
    pub overrides: PresetOverride,
}

/// Ordered preset composition pipeline.
///
/// Collects layers and applies them onto a base `Preset` in priority order.
/// Layers of the same type are applied in insertion order.
#[derive(Debug, Clone, Default)]
pub struct PresetPipeline {
    layers: Vec<PresetLayer>,
}

impl PresetPipeline {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a layer to the pipeline.
    pub fn add(&mut self, layer: PresetLayer) {
        self.layers.push(layer);
    }

    /// Add a layer only if the override is present (Option-based convenience).
    pub fn add_optional(&mut self, layer: Option<PresetLayer>) {
        if let Some(l) = layer {
            self.layers.push(l);
        }
    }

    /// How many layers are in the pipeline.
    #[must_use]
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Whether the pipeline has no layers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Apply all layers onto a base preset, in layer-type order.
    ///
    /// Layers are sorted by `PresetLayerType` (ascending). Within the same
    /// type, insertion order is preserved (stable sort).
    #[must_use]
    pub fn apply(mut self, base: Preset) -> Preset {
        self.layers.sort_by_key(|l| l.layer_type);
        self.layers
            .into_iter()
            .fold(base, |preset, layer| preset.merge(&layer.overrides))
    }

    /// Return the sorted layer names (for debugging/logging).
    pub fn layer_names(&self) -> Vec<String> {
        let mut layers = self.layers.clone();
        layers.sort_by_key(|l| l.layer_type);
        layers.into_iter().map(|l| l.name).collect()
    }
}
