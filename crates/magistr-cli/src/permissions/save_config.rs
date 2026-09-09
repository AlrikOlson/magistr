//! Save permission config to a project directory.

use std::path::Path;

use super::config_path::config_path;
use super::permission_config::PermissionConfig;
use crate::error::{MagistrError, Result};

/// Save permission config to a project directory.
///
/// Creates the `.claude/` directory if it doesn't exist.
pub fn save_config(project_dir: &Path, config: &PermissionConfig) -> Result<()> {
    let path = config_path(project_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
    }

    let toml = config.to_toml()?;
    std::fs::write(&path, toml).map_err(|e| MagistrError::io(&path, e))?;

    Ok(())
}
