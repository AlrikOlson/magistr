//! Orchestration domain — workflow engine, plan management, and CLI commands.
//!
//! Contains the iteration lifecycle (plan → execute → verify), check
//! runners, plan parsing/tracking, and all CLI command handlers.

pub mod commands;
pub mod goal;
pub(crate) mod hosted_work;
pub(crate) mod interactive_session;
pub mod ledger;
pub mod loop_driver;
pub mod plan;
pub(crate) mod plan_request;
mod shared_checkpoint;
pub mod workflow;
