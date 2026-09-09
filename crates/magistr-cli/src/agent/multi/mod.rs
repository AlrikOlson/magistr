//! Multi-agent orchestration — manage multiple concurrent ACP sessions.
//!
//! Provides the infrastructure for spawning, monitoring, and coordinating
//! multiple agent workers under a supervisor. Key components:
//!
//! - [`topology`] — Agent roles, IDs, and inter-agent message protocol
//! - [`pool`] — Lifecycle management for concurrent agent sessions
//! - [`supervisor`] — Task routing, result collection, failure handling
//! - [`partition`] — Work partitioning strategies (by file, phase, component)
//! - [`conflict`] — Conflict detection when multiple agents edit the same file
//! - [`context`] — Shared knowledge base across agents
//! - [`budget`] — Per-agent and aggregate resource budgeting
//! - [`preset`] — Predefined orchestration patterns
//! - [`canvas`] — Visual canvas data model for topology visualization

pub mod archetype_conflict;
pub mod archetype_context;
pub mod archetype_verify;
pub mod budget;
pub mod canvas;
pub mod conflict;
pub mod context;
pub mod partition;
pub mod pattern;
pub mod pool;
pub mod preset;
pub mod role;
pub mod role_dispatch;
pub mod role_loader;
pub mod role_recommend;
pub mod supervisor;
pub mod tool_policy;
pub mod topology;

#[cfg(test)]
#[path = "orchestration_integration.tests.rs"]
mod orchestration_integration_tests;
