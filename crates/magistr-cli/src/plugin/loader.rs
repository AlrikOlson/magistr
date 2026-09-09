//! Plugin loading — read manifests and configs from plugin directories.
//!
//! Loads a plugin from a local directory by reading `magistr-plugin.toml`
//! and any associated config files (`detect.toml`, `templates.toml`).

use std::path::Path;

use super::types::{
    DetectionConfig, MANIFEST_FILENAME, PluginError, PluginKind, PluginManifest, TemplateConfig,
};

/// Load a plugin manifest from a directory.
///
/// Expects `<dir>/magistr-plugin.toml` to exist and be valid TOML.
pub fn load_manifest(dir: &Path) -> Result<PluginManifest, PluginError> {
    if !dir.is_dir() {
        return Err(PluginError::DirNotFound(dir.display().to_string()));
    }

    let manifest_path = dir.join(MANIFEST_FILENAME);
    if !manifest_path.is_file() {
        return Err(PluginError::ManifestNotFound(dir.display().to_string()));
    }

    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| PluginError::Io(manifest_path.display().to_string(), e))?;

    PluginManifest::from_toml(&content)
}

/// Load detection config from a plugin directory.
///
/// Reads `<dir>/detect.toml`. Returns empty config if file doesn't exist.
pub fn load_detection_config(dir: &Path) -> Result<DetectionConfig, PluginError> {
    let path = dir.join("detect.toml");
    if !path.is_file() {
        return Ok(DetectionConfig { rules: Vec::new() });
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| PluginError::Io(path.display().to_string(), e))?;

    DetectionConfig::from_toml(&content)
}

/// Load template config from a plugin directory.
///
/// Reads `<dir>/templates.toml`. Returns empty config if file doesn't exist.
pub fn load_template_config(dir: &Path) -> Result<TemplateConfig, PluginError> {
    let path = dir.join("templates.toml");
    if !path.is_file() {
        return Ok(TemplateConfig { files: Vec::new() });
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| PluginError::Io(path.display().to_string(), e))?;

    TemplateConfig::from_toml(&content)
}

/// Validate that a plugin directory has the correct structure.
///
/// Checks:
/// - Manifest exists and is valid
/// - Manifest-level validation (entrypoint requirements, etc.)
/// - Detection plugins have `detect.toml`
/// - Template plugins have `templates.toml` and referenced files exist
/// - Executable plugins have a valid entrypoint file
pub fn validate_plugin(dir: &Path) -> Result<Vec<String>, PluginError> {
    let manifest = load_manifest(dir)?;
    let mut warnings = Vec::new();

    // Run manifest-level validation
    let validation_errors = manifest.validate();
    if !validation_errors.is_empty() {
        return Err(PluginError::ManifestValidation(validation_errors));
    }

    match manifest.kind {
        PluginKind::Detection => {
            let detect_path = dir.join("detect.toml");
            if !detect_path.is_file() {
                warnings.push(
                    "detection plugin missing detect.toml — no rules will be loaded".to_string(),
                );
            } else {
                let config = load_detection_config(dir)?;
                if config.rules.is_empty() {
                    warnings.push("detect.toml has no rules defined".to_string());
                }
            }
        }
        PluginKind::Template => {
            let templates_path = dir.join("templates.toml");
            if !templates_path.is_file() {
                warnings.push(
                    "template plugin missing templates.toml — no files will be scaffolded"
                        .to_string(),
                );
            } else {
                let config = load_template_config(dir)?;
                for file in &config.files {
                    let src_path = dir.join(&file.src);
                    if !src_path.is_file() {
                        warnings.push(format!(
                            "template references '{}' but file not found",
                            file.src
                        ));
                    }
                }
            }
        }
        PluginKind::WorkflowStep | PluginKind::QualityGate => {
            // Executable plugins: verify entrypoint file exists
            if let Some(ref entrypoint) = manifest.entrypoint {
                let ep_path = dir.join(entrypoint);
                if !ep_path.is_file() {
                    warnings.push(format!(
                        "entrypoint '{}' not found in plugin directory",
                        entrypoint,
                    ));
                }
            }
        }
    }

    Ok(warnings)
}

#[cfg(test)]
#[path = "loader.tests.rs"]
mod tests;
