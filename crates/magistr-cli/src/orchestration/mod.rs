//! Orchestration domain — workflow engine, roadmap management, and CLI commands.
//!
//! Contains the iteration lifecycle (plan → execute → verify), quality gate
//! runners, roadmap parsing/tracking, and all CLI command handlers.

pub mod commands;
pub mod goal;
pub mod ledger;
pub mod loop_driver;
pub mod roadmap;
pub mod workflow;
