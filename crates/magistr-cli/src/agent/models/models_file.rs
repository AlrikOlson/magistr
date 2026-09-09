//! Top-level models configuration file structure.

use serde::{Deserialize, Serialize};

use super::model_config::ModelConfig;

/// Top-level structure of `~/.config/magistr/models.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelsFile {
    /// User-configured models (keyed by model id).
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}
