//! Which roadmap backend a project directory effectively uses.
//!
//! `think-and-ship` is the default, but a project that predates the
//! `roadmap_backend` key must keep reading its YAML roadmap until it is
//! migrated — flipping a default is only safe when the old projects cannot
//! change behaviour silently. Every loader resolves the backend through
//! [`backend_for`] so there is exactly one rule.

use std::path::Path;

use magistr_engine::native::RoadmapBackend;

/// The backend `.magistr/project.json` declares, if it declares one.
#[must_use]
pub fn declared_backend(project_dir: &Path) -> Option<RoadmapBackend> {
    let raw = std::fs::read_to_string(crate::paths::project_json(project_dir).ok()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    serde_json::from_value(value.get("roadmap_backend")?.clone()).ok()
}

/// The effective backend for `project_dir`.
///
/// A declared value wins. Without one, the project is `Yaml` when it
/// predates the key — a YAML roadmap exists at `.magistr-roadmap.yml` or the
/// legacy `.magistr/roadmap.yaml`, the configured `spec.file` exists, or
/// there is no `.magistr/project.json` at all (a spec-file-only project).
/// Otherwise it is the default, `ThinkAndShip`.
#[must_use]
pub fn backend_for(project_dir: &Path) -> RoadmapBackend {
    backend_for_declared(project_dir, declared_backend(project_dir))
}

/// [`backend_for`] with the declared value already in hand (loaders that
/// have parsed `project.json` pass what they saw).
#[must_use]
pub fn backend_for_declared(
    project_dir: &Path,
    declared: Option<RoadmapBackend>,
) -> RoadmapBackend {
    if let Some(declared) = declared {
        return declared;
    }
    let Ok(config_path) = crate::paths::project_json(project_dir) else {
        return RoadmapBackend::default();
    };
    if !config_path.exists() {
        return RoadmapBackend::Yaml;
    }
    if super::yaml_path(project_dir).exists() || super::legacy_yaml_path(project_dir).exists() {
        return RoadmapBackend::Yaml;
    }
    if spec_file(&config_path).is_some_and(|spec| project_dir.join(spec).exists()) {
        return RoadmapBackend::Yaml;
    }
    RoadmapBackend::default()
}

/// `spec.file` from a project.json, when present.
fn spec_file(config_path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(config_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value.get("spec")?.get("file")?.as_str().map(str::to_owned)
}

#[cfg(test)]
#[path = "backend_for.tests.rs"]
mod tests;
