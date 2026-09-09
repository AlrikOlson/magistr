//! Model registry — re-exports from split submodules.

pub use super::model_entry::ModelEntry;
pub use super::model_registry::ModelRegistry;
pub use super::model_source::ModelSource;

#[cfg(test)]
#[path = "registry.tests.rs"]
mod tests;
