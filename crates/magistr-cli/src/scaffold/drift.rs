//! Drift detection — compare current filesystem state against existing config.
//!
//! Re-runs the project tree detection and compares against the configured
//! archetypes to surface new projects, removed projects, and framework/language
//! changes.

use std::collections::HashSet;
use std::path::PathBuf;

use magistr_core::types::{Archetype, DetectedProject, ProjectTree};

/// A single detected change between config and filesystem state.
#[derive(Debug, Clone)]
pub struct DriftChange {
    /// Archetype id this applies to.
    pub archetype_id: String,
    /// Human-readable description of what changed.
    pub description: String,
}

/// Result of comparing filesystem state against existing config.
#[derive(Debug, Clone)]
pub struct DriftReport {
    /// Projects detected on disk but not in config.
    pub new_projects: Vec<DetectedProject>,
    /// Archetype ids in config but not found on disk.
    pub removed_archetypes: Vec<String>,
    /// Changes to existing archetypes (language, framework, etc.).
    pub changes: Vec<DriftChange>,
}

impl DriftReport {
    /// Whether the config matches the filesystem exactly.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.new_projects.is_empty()
            && self.removed_archetypes.is_empty()
            && self.changes.is_empty()
    }

    /// Total number of drifts detected.
    #[must_use]
    pub fn total(&self) -> usize {
        self.new_projects.len() + self.removed_archetypes.len() + self.changes.len()
    }
}

/// Compare configured archetypes against a freshly-detected project tree.
pub fn detect_drift(archetypes: &[Archetype], tree: &ProjectTree) -> DriftReport {
    let config_roots: HashSet<PathBuf> =
        archetypes.iter().map(|a| normalize_root(&a.root)).collect();

    let detected_roots: HashSet<PathBuf> = tree
        .projects
        .iter()
        .map(|p| normalize_root(&p.path))
        .collect();

    // New projects: in tree but not in config
    let new_projects: Vec<DetectedProject> = tree
        .projects
        .iter()
        .filter(|p| !config_roots.contains(&normalize_root(&p.path)))
        .cloned()
        .collect();

    // Removed archetypes: in config but not in tree
    let removed_archetypes: Vec<String> = archetypes
        .iter()
        .filter(|a| !detected_roots.contains(&normalize_root(&a.root)))
        .map(|a| a.id.clone())
        .collect();

    // Changes to matched projects
    let mut changes = Vec::new();
    for arch in archetypes {
        let root = normalize_root(&arch.root);
        if let Some(detected) = tree
            .projects
            .iter()
            .find(|p| normalize_root(&p.path) == root)
        {
            let detected_lang = detected.language.as_str();
            if arch.language != detected_lang {
                changes.push(DriftChange {
                    archetype_id: arch.id.clone(),
                    description: format!("language changed: {} → {}", arch.language, detected_lang),
                });
            }

            let detected_fw = detected.framework.map(|f| f.as_str().to_string());
            if arch.framework != detected_fw {
                let old = arch.framework.as_deref().unwrap_or("none");
                let new = detected_fw.as_deref().unwrap_or("none");
                changes.push(DriftChange {
                    archetype_id: arch.id.clone(),
                    description: format!("framework changed: {old} → {new}"),
                });
            }
        }
    }

    DriftReport {
        new_projects,
        removed_archetypes,
        changes,
    }
}

/// Normalize a root path: empty or "." both become ".".
fn normalize_root(path: &std::path::Path) -> PathBuf {
    if path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
#[path = "drift.tests.rs"]
mod tests;
