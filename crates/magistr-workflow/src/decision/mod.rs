//! Decision log domain and the `decision_*` tool family.
//!
//! The body of this module is ported from the standalone reasoning server with
//! peer-module references rewritten to `crate::decision::*`. The tool wire
//! wiring (the `mcp::*` adapter) lives in the `mcp` module.

#![allow(clippy::collapsible_if)]

pub mod application;
pub mod broadcast;
pub mod config;
mod config_resolution;
mod config_source;
pub mod constants;
pub mod domain;
pub mod engine;
pub mod formatter;
pub mod mcp;
pub mod output_schemas;
pub mod persistence;
pub mod util;

pub use config_resolution::DecisionConfigResolution;
pub use config_source::ConfigSource;
pub use mcp::service::DecisionService;
