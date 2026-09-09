//! Stack detection — canonical definitions live in `magistr-core`.
//!
//! This module re-exports core detection from magistr-core and adds
//! plugin-based detection integration that depends on the plugin system.

pub use magistr_core::detection::detect::*;

use std::path::Path;

use tracing::{debug, instrument};

use crate::error::Result;
use crate::plugin;
use crate::types::PluginDetection;

/// Detect the full project stack, including plugin detections.
///
/// Wraps the core `detect_stack` from magistr-core and enriches the result
/// with detections from installed plugins.
#[instrument(skip_all, fields(dir = %dir.display()))]
pub fn detect_stack_full(dir: &Path) -> Result<crate::types::Stack> {
    let plugins = run_plugin_detections(dir);
    magistr_core::detection::detect::detect_stack_with_plugins(dir, plugins)
}

/// Run all installed detection plugins against a project directory.
///
/// Loads the plugin registry, iterates detection plugins, evaluates their rules
/// against the project directory, and returns any matching detections.
/// Failures (missing registry, bad plugin config) are silently ignored —
/// plugin detection is best-effort and should never block built-in detection.
#[instrument(skip_all, fields(dir = %dir.display()))]
fn run_plugin_detections(dir: &Path) -> Vec<PluginDetection> {
    let base_dir = match plugin::plugins_dir() {
        Some(d) => d,
        None => {
            debug!("no plugins directory found");
            return Vec::new();
        }
    };

    let registry = match plugin::PluginRegistry::load(&base_dir) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut detections = Vec::new();

    for installed in registry.list() {
        if installed.manifest.kind != plugin::PluginKind::Detection {
            continue;
        }

        let plugin_dir = registry.plugin_dir(&installed.manifest.name);
        let config = match plugin::load_detection_config(&plugin_dir) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for rule in &config.rules {
            if matches_detection_rule(dir, rule) {
                debug!(
                    plugin = %installed.manifest.name,
                    category = %rule.category,
                    name = %rule.name,
                    "plugin detection matched"
                );
                detections.push(PluginDetection {
                    category: rule.category.clone(),
                    name: rule.name.clone(),
                    confidence: rule.confidence,
                    plugin_name: installed.manifest.name.clone(),
                });
            }
        }
    }

    detections
}

/// Check if a detection rule matches a project directory.
fn matches_detection_rule(dir: &Path, rule: &plugin::DetectionRule) -> bool {
    if rule.file_patterns.is_empty() && rule.content_patterns.is_empty() {
        return false;
    }

    let matched_files: Vec<std::path::PathBuf> = if rule.file_patterns.is_empty() {
        std::fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect()
    } else {
        rule.file_patterns
            .iter()
            .filter_map(|pattern| {
                let path = dir.join(pattern);
                if path.exists() { Some(path) } else { None }
            })
            .collect()
    };

    if matched_files.is_empty() && !rule.file_patterns.is_empty() {
        return false;
    }

    if rule.content_patterns.is_empty() {
        return !matched_files.is_empty();
    }

    for file in &matched_files {
        if let Ok(content) = std::fs::read_to_string(file) {
            for pattern in &rule.content_patterns {
                if content.contains(pattern.as_str()) {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
