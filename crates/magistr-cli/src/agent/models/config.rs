//! Model configuration — re-exports from split submodules.
//!
//! This module currently re-exports model types for local callers and tests.
//! Redundant import paths may be removed with their callers.

pub use super::builtin_models::builtin_models;
pub use super::load::load;
pub use super::load_from::load_from;
pub use super::model_config::ModelConfig;
pub use super::model_config_error::ModelConfigError;
pub use super::models_file::ModelsFile;
pub use super::provider::Provider;
pub use super::save_to::save_to;

#[cfg(test)]
#[path = "config.tests.rs"]
mod tests;
