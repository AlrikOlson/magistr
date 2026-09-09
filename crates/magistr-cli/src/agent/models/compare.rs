//! Model comparison — re-exports from split submodules.

#![allow(dead_code)] // Public API — wired from CLI `magistr compare` command

pub use super::comparison_request::ComparisonRequest;
pub use super::comparison_result::ComparisonResult;
pub use super::model_response::ModelResponse;
pub use super::render_comparison::render_comparison;
pub use super::render_comparison_json::render_comparison_json;

#[cfg(test)]
#[path = "compare.tests.rs"]
mod tests;
