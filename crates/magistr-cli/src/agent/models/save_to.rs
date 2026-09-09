//! Save model configs to a file path.

use std::path::Path;

use super::model_config_error::ModelConfigError;
use super::models_file::ModelsFile;

/// Save model configs to a specific file path (for testing and `magistr models init`).
pub fn save_to(file: &ModelsFile, path: &Path) -> Result<(), ModelConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ModelConfigError::Io(parent.to_owned(), e))?;
    }

    let content =
        toml::to_string_pretty(file).map_err(|e| ModelConfigError::Serialize(e.to_string()))?;
    std::fs::write(path, content).map_err(|e| ModelConfigError::Io(path.to_owned(), e))?;
    Ok(())
}
