//! Orchestration domain — workflow engine, roadmap management, and CLI commands.
//!
//! Contains the iteration lifecycle (plan → execute → verify), quality gate
//! runners, roadmap parsing/tracking, and all CLI command handlers.

pub mod commands;
pub mod goal;
pub(crate) mod hosted_work;
pub(crate) mod interactive_session;
pub mod ledger;
pub mod loop_driver;
pub mod roadmap;
pub(crate) mod roadmap_request;
mod shared_checkpoint;
pub mod workflow;
