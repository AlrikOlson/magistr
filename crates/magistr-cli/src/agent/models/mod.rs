//! Multi-model configuration, registry, smart routing, and comparison.
//!
//! Provides:
//! - **Config** — `ModelConfig`, `ModelsFile`, TOML parsing, built-in defaults
//! - **Registry** — `ModelRegistry` merging builtins with user config
//! - **Routing** — Smart model routing based on task complexity estimation
//! - **Compare** — Side-by-side model comparison with cost analysis

// ── Config ─────────────────────────────────────────────────────────
pub mod advertised_pricing;
pub mod builtin_models;
pub mod load;
pub mod load_from;
pub mod model_config;
pub mod model_config_error;
pub mod models_file;
pub mod provider;
pub mod save_to;

// ── Registry ───────────────────────────────────────────────────────
pub mod model_entry;
pub mod model_registry;
pub mod model_source;

// ── Routing ────────────────────────────────────────────────────────
pub mod complexity_estimate;
pub mod estimate_complexity;
pub mod route;
pub mod routing_decision;
pub mod routing_log;
pub mod task_complexity;

// ── Compare ────────────────────────────────────────────────────────
pub mod comparison_request;
pub mod comparison_result;
pub mod model_response;
pub mod render_comparison;
pub mod render_comparison_json;

// ── Current host test module exports ─────────────────────────────
pub mod compare;
pub mod config;
pub mod registry;
pub mod routing;

// ── Re-exports ─────────────────────────────────────────────────────
#[allow(unused_imports)]
pub use advertised_pricing::advertised_pricing;
#[allow(unused_imports)]
pub use builtin_models::builtin_models;
#[allow(unused_imports)]
pub use comparison_request::ComparisonRequest;
#[allow(unused_imports)]
pub use comparison_result::ComparisonResult;
#[allow(unused_imports)]
pub use complexity_estimate::ComplexityEstimate;
#[allow(unused_imports)]
pub use estimate_complexity::estimate_complexity;
#[allow(unused_imports)]
pub use load::load;
#[allow(unused_imports)]
pub use load_from::load_from;
#[allow(unused_imports)]
pub use model_config::ModelConfig;
#[allow(unused_imports)]
pub use model_config_error::ModelConfigError;
#[allow(unused_imports)]
pub use model_entry::ModelEntry;
#[allow(unused_imports)]
pub use model_registry::ModelRegistry;
#[allow(unused_imports)]
pub use model_response::ModelResponse;
#[allow(unused_imports)]
pub use model_source::ModelSource;
#[allow(unused_imports)]
pub use provider::Provider;
#[allow(unused_imports)]
pub use render_comparison::render_comparison;
#[allow(unused_imports)]
pub use render_comparison_json::render_comparison_json;
#[allow(unused_imports)]
pub use route::route;
#[allow(unused_imports)]
pub use routing_decision::RoutingDecision;
#[allow(unused_imports)]
pub use routing_log::RoutingLog;
#[allow(unused_imports)]
pub use save_to::save_to;
#[allow(unused_imports)]
pub use task_complexity::TaskComplexity;
