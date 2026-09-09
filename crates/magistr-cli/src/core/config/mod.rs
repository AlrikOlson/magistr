//! Configuration — canonical definitions live in `magistr-core`.
//!
//! All submodules are re-exported here so existing `crate::config::*` imports
//! continue to work without changes.

pub use magistr_core::config::*;

/// CLI-specific migration execution (depends on `crate::roadmap`).
pub mod migration_exec;
