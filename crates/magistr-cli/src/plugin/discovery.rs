//! Plugin discovery — scan filesystem for installed plugins.
//!
//! Scans two locations for plugins:
//! 1. Project-local: `.magistr/plugins/` relative to the project root
//! 2. Global: the captured plugins directory (`ProjectInputs::plugins_dir`,
//!    normally `~/.config/magistr/plugins/`)
//!
//! Project-local plugins take precedence over global plugins with the same name.

use std::path::{Path, PathBuf};

use super::loader;
use super::types::{DiscoveredPlugin, PluginScope};

/// Discover all available plugins for a project.
///
/// Scans both the project-local `.magistr/plugins/` directory and the global
/// plugins directory, when the caller resolved one. When the same plugin name
/// appears in both locations, the project-local version takes precedence.
///
/// Plugins with invalid manifests are silently skipped — discovery is
/// best-effort and should never block the host.
pub fn discover_plugins(project_dir: &Path, global_dir: Option<&Path>) -> Vec<DiscoveredPlugin> {
    let mut plugins = Vec::new();

    // Scan project-local first (higher priority)
    let local_dir = crate::paths::plugins_dir(project_dir);
    scan_directory(&local_dir, PluginScope::ProjectLocal, &mut plugins);

    // Scan global directory
    if let Some(global_dir) = global_dir {
        scan_directory(global_dir, PluginScope::Global, &mut plugins);
    }

    plugins
}

/// Discover only project-local plugins.
pub fn discover_local_plugins(project_dir: &Path) -> Vec<DiscoveredPlugin> {
    let mut plugins = Vec::new();
    let local_dir = crate::paths::plugins_dir(project_dir);
    scan_directory(&local_dir, PluginScope::ProjectLocal, &mut plugins);
    plugins
}

/// Discover only the plugins under the captured global directory.
pub fn discover_global_plugins(global_dir: &Path) -> Vec<DiscoveredPlugin> {
    let mut plugins = Vec::new();
    scan_directory(global_dir, PluginScope::Global, &mut plugins);
    plugins
}

/// Scan a directory for plugin subdirectories.
///
/// Each immediate child directory is tested for a `magistr-plugin.toml` manifest.
/// Valid plugins are appended to `out`, skipping duplicates (by name) that
/// are already present from a higher-priority scan.
fn scan_directory(dir: &Path, scope: PluginScope, out: &mut Vec<DiscoveredPlugin>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return, // Directory doesn't exist or isn't readable
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest = match loader::load_manifest(&path) {
            Ok(m) => m,
            Err(_) => continue, // Invalid/missing manifest — skip
        };

        // Skip if a higher-priority plugin with the same name already exists
        if out.iter().any(|p| p.manifest.name == manifest.name) {
            continue;
        }

        out.push(DiscoveredPlugin {
            manifest,
            path,
            scope,
        });
    }
}

/// Resolve the absolute path to the project-local plugins directory.
#[must_use]
pub fn local_plugins_dir(project_dir: &Path) -> PathBuf {
    crate::paths::plugins_dir(project_dir)
}

#[cfg(test)]
#[path = "discovery.tests.rs"]
mod tests;
