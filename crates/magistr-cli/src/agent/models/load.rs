//! Load model configs from the captured models path.

use std::path::Path;

use super::load_from::load_from;
use super::model_config_error::ModelConfigError;
use super::models_file::ModelsFile;

/// Load model configs from the path the captured inputs resolved
/// (`ProjectInputs::models_config_path`).
///
/// Returns `ModelsFile::default()` when no path could be resolved or the
/// file does not exist; an existing file that cannot be parsed is an error.
pub fn load(path: Option<&Path>) -> Result<ModelsFile, ModelConfigError> {
    match path {
        Some(path) => load_from(path),
        None => Ok(ModelsFile::default()),
    }
}
