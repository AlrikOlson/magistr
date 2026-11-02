//! Canonical YAML path resolution for plan files.

use std::path::{Path, PathBuf};

/// Resolve the canonical YAML path for a project directory.
///
/// The plan lives at the repo root as `.magistr-plan.yml` — a single
/// YAML file that is the source of truth. No markdown is auto-generated.
#[must_use]
pub fn yaml_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".magistr-plan.yml")
}
