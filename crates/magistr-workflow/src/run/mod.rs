//! Execution tracking domain and the `run_*` tool family.
//!
//! The body of this module is ported from the standalone execution server with
//! peer-module references rewritten to `crate::run::*`. Tool wire wiring
//! lives in the `mcp` submodule; broadcast routes through the shared
//! `crate::infra::Broadcaster` with `Family::Run`.

pub mod application;
pub mod approval;
pub mod broadcast;
pub mod check_result_compaction;
pub mod check_result_ledger;
pub mod check_result_storage;
pub mod domain;
pub mod engine;
pub mod mcp;
pub mod output_schemas;
pub mod persistence;
pub mod report;

pub use application::RunUseCases;
pub use mcp::service::RunService;

mod saved_cycle;
