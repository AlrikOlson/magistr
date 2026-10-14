//! magistr — AI-powered development toolkit for the terminal.
//!
//! This library crate re-exports internal modules for integration testing.
//! The binary entry point is in `main.rs`.

// ── Domain modules ──────────────────────────────────────────────────────────

pub mod core;
pub mod detection;
pub mod orchestration;

// ── Current module re-exports ──────────────────────────────────────────────
//
// These re-exports preserve all existing `crate::X` import paths so that
// moving modules into domain boundaries is a zero-churn migration.
// Import path modernization (crate::core::error, etc.) happens incrementally.

pub use self::core::{config, env, error, paths, types};
pub use self::detection::{detect, lint, validate};
pub use self::orchestration::{commands, roadmap, workflow};

// ── Agent domain (consolidated: agent, engine, models, session) ─────────────

pub mod agent;
pub mod application;

pub use self::agent::models;
pub use self::agent::persistence as session;

// ── Remaining top-level modules (not yet reorganized) ───────────────────────

pub mod cli;
pub mod codex_launch;
pub mod context;
pub mod cost;
pub mod headless;
mod mcp_bridge;
pub mod mcp_target;
pub mod mcp_tools;
pub mod memory;
pub mod output;
pub mod permissions;
pub mod plugin;
pub mod scaffold;
pub mod team;
pub mod tools;
pub mod utils;
pub mod workspace_cli;
pub mod workspace_mcp;

// ── Test infrastructure (test-only) ─────────────────────────────────────────

#[cfg(test)]
pub mod test_support;

#[cfg(test)]
#[path = "cli.test_support.rs"]
pub(crate) mod cli_test_support;

#[cfg(test)]
#[path = "../tests/support/in_env.rs"]
pub(crate) mod in_env;
pub mod workflow_agent_options;
mod working_copy_action;
