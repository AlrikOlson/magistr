//! Full preset loading with all layers:
//! built-in → framework → package-manager → database → CI/CD →
//! deployment → project-local → user override.

use std::path::Path;

use crate::error::Result;
use crate::types::{
    Framework, Language, PackageManager, Preset, PresetLayer, PresetLayerType, PresetOverride,
    PresetPipeline,
};

/// Parse a JSON overlay and add it to the pipeline if present.
fn add_json_overlay(
    pipeline: &mut PresetPipeline,
    json: Option<&str>,
    layer_type: PresetLayerType,
    name: &str,
    label: &str,
) -> Result<()> {
    if let Some(json) = json {
        let overrides = serde_json::from_str(json).map_err(|e| {
            crate::error::MagistrError::Config(format!("{label} overlay parse error: {e}"))
        })?;
        pipeline.add(PresetLayer {
            layer_type,
            name: name.to_string(),
            overrides,
        });
    }
    Ok(())
}

/// Load a preset with full layering: built-in → framework → package-manager → user override.
///
/// This is the backwards-compatible entry point. For the full layered pipeline
/// including database, CI/CD, deployment, and project-local layers, use
/// [`load_preset_layered`].
pub fn load_preset_full(
    lang: Language,
    framework: Framework,
    package_manager: PackageManager,
) -> Result<Preset> {
    load_preset_layered(lang, framework, package_manager, &LayerOptions::default())
}

/// Options for the extended layered preset pipeline.
#[derive(Debug, Clone, Default)]
pub struct LayerOptions {
    /// Database/ORM slug (e.g., "prisma", "diesel", "sqlx").
    pub database: Option<String>,
    /// CI/CD provider slug (e.g., "github-actions", "gitlab-ci").
    pub ci: Option<String>,
    /// Deployment target slug (e.g., "docker", "kubernetes", "vercel").
    pub deployment: Option<String>,
    /// Project root for loading `.magistr/presets/` fragments.
    pub project_root: Option<Box<Path>>,
}

/// Load a preset with the full layered composition pipeline.
///
/// Layers are applied in order: language → framework → package-manager →
/// database → CI/CD → deployment → project-local → user override.
/// Missing layers are gracefully skipped.
pub fn load_preset_layered(
    lang: Language,
    framework: Framework,
    package_manager: PackageManager,
    options: &LayerOptions,
) -> Result<Preset> {
    load_preset_layered_with(
        lang,
        framework,
        package_manager,
        options,
        super::user_preset::load_user_override,
    )
}

/// Keep the layered composition independent of the user's config directory.
fn load_preset_layered_with(
    lang: Language,
    framework: Framework,
    package_manager: PackageManager,
    options: &LayerOptions,
    load_user_override: impl FnOnce(Language) -> Result<Option<PresetOverride>>,
) -> Result<Preset> {
    // Start with the base language preset
    let base = super::preset::load_preset(lang)?;

    // Build the pipeline with all available layers
    let mut pipeline = PresetPipeline::new();

    // Framework overlay
    add_json_overlay(
        &mut pipeline,
        super::preset_framework::framework_override_json_pub(framework),
        PresetLayerType::Framework,
        &format!("{framework:?}"),
        "framework",
    )?;

    // Package-manager overlay
    add_json_overlay(
        &mut pipeline,
        super::preset_framework::pm_override_json_pub(package_manager),
        PresetLayerType::PackageManager,
        &format!("{package_manager:?}"),
        "pm",
    )?;

    // Database/ORM layer
    if let Some(slug) = &options.database {
        pipeline.add_optional(super::preset_layers::load_database_layer(slug)?);
    }

    // CI/CD layer
    if let Some(slug) = &options.ci {
        pipeline.add_optional(super::preset_layers::load_ci_layer(slug)?);
    }

    // Deployment layer
    if let Some(slug) = &options.deployment {
        pipeline.add_optional(super::preset_layers::load_deployment_layer(slug)?);
    }

    // Project-local layers
    if let Some(root) = &options.project_root {
        let project_layers = super::project_preset::load_project_presets(root)?;
        for layer in project_layers {
            pipeline.add(layer);
        }
    }

    // User-global override
    if let Some(user_override) = load_user_override(lang)? {
        pipeline.add(PresetLayer {
            layer_type: PresetLayerType::UserOverride,
            name: format!("user-{}", lang.as_str()),
            overrides: user_override,
        });
    }

    Ok(pipeline.apply(base))
}

#[cfg(test)]
#[path = "preset_full.tests.rs"]
mod tests;
