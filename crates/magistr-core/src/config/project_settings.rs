//! Read the current project settings without changing their contents.

use std::path::Path;

use crate::error::{MagistrError, Result};
use crate::types::ProjectConfig;

/// Read settings at the resolved project settings path.
///
/// Only an absent file returns `None`. Existing settings must be readable and
/// satisfy the current configuration contract; defaults apply to omitted fields.
pub fn read_project_settings(path: &Path) -> Result<Option<ProjectConfig>> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if matches!(std::fs::symlink_metadata(path), Err(missing) if missing.kind() == std::io::ErrorKind::NotFound)
            {
                return Ok(None);
            }
            return Err(MagistrError::io(path, error));
        }
        Err(error) => return Err(MagistrError::io(path, error)),
    };
    let upgraded = super::ProjectSettingsUpgrade::prepare(&content).map_err(|error| {
        MagistrError::Config(format!(
            "Invalid project settings at {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_str(upgraded.as_deref().unwrap_or(&content))
        .map(Some)
        .map_err(|error| {
            MagistrError::Config(format!(
                "Invalid project settings at {}: {error}",
                path.display()
            ))
        })
}

#[cfg(test)]
#[path = "project_settings.tests.rs"]
mod tests;
