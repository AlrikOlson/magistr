//! Resolve the path to the permissions config file.

use std::path::{Path, PathBuf};

/// Resolve the path to the permissions config file in a project.
pub fn config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".claude").join("permissions.toml")
}
