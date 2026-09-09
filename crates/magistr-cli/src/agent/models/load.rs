//! Load model configs from the default path.

use super::load_from::load_from;
use super::model_config_error::ModelConfigError;
use super::models_config_path::models_config_path;
use super::models_file::ModelsFile;

/// Load model configs from the default path.
///
/// Returns `ModelsFile::default()` if the file does not exist.
pub fn load() -> Result<ModelsFile, ModelConfigError> {
    let path = match models_config_path() {
        Some(p) => p,
        None => return Ok(ModelsFile::default()),
    };
    load_from(&path)
}
