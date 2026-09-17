//! Model configuration — re-exports from split submodules.
//!
//! This module re-exports all types that were previously defined here,
//! preserving backward compatibility for `use super::config::*` in tests.

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
