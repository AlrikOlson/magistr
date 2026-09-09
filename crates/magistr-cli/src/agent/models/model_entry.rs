//! A model entry in the registry with its source.

use super::model_config::ModelConfig;
use super::model_source::ModelSource;

/// A model entry in the registry with its source.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEntry {
    /// The model configuration.
    pub config: ModelConfig,
    /// Where this model definition came from.
    pub source: ModelSource,
}
