//! Load permission config from a project directory.

use std::path::Path;

use super::config_path::config_path;
use super::permission_config::PermissionConfig;
use crate::error::Result;

/// Load permission config from a project directory.
///
/// Returns `Default` if the file doesn't exist.
/// Returns an error if the file exists but is invalid.
pub fn load_config(project_dir: &Path) -> Result<PermissionConfig> {
    let path = config_path(project_dir);
    if !path.exists() {
        return Ok(PermissionConfig::default());
    }

    let content =
        std::fs::read_to_string(&path).map_err(|e| crate::error::MagistrError::io(&path, e))?;
    PermissionConfig::from_toml(&content)
}
