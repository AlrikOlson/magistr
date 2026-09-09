//! Canonical YAML path resolution for roadmap files.

use std::path::{Path, PathBuf};

/// Resolve the canonical YAML path for a project directory.
///
/// The roadmap lives at the repo root as `.magistr-roadmap.yml` — a single
/// YAML file that is the source of truth. No markdown is auto-generated.
#[must_use]
pub fn yaml_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".magistr-roadmap.yml")
}

/// Legacy YAML path used before the repo-root migration.
///
/// Checked by [`super::yaml_store::YamlStore::load_or_create`] for
/// transparent migration of existing projects.
#[must_use]
pub fn legacy_yaml_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".magistr").join("roadmap.yaml")
}
