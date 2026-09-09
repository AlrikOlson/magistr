//! Load model configs from a specific file path.

use std::path::Path;

use super::model_config_error::ModelConfigError;
use super::models_file::ModelsFile;

/// Load model configs from a specific file path.
///
/// Returns an empty `ModelsFile` if the file does not exist.
/// Returns an error only if the file exists but is invalid TOML.
pub fn load_from(path: &Path) -> Result<ModelsFile, ModelConfigError> {
    if !path.is_file() {
        return Ok(ModelsFile::default());
    }

    let content =
        std::fs::read_to_string(path).map_err(|e| ModelConfigError::Io(path.to_owned(), e))?;
    let file: ModelsFile = toml::from_str(&content)
        .map_err(|e| ModelConfigError::Parse(path.to_owned(), e.to_string()))?;
    Ok(file)
}
