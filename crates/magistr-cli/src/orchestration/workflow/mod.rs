//! Autonomous workflow engine — canonical definitions live in `magistr-core`.
//!
//! This module re-exports the workflow system from magistr-core for
//! backward-compatible `crate::workflow::*` import paths, and adds
//! async runtime-dependent modules (gate_runner, hook_dispatch, watcher).

// ── Re-export everything from magistr-core::workflow ────────────────────────

pub use magistr_core::workflow::*;

// ── CLI-only modules (async runtime, TUI deps) ─────────────────────────────

pub mod gate_runner;
pub mod hook_dispatch;
pub mod watcher;
