//! CLAUDE.md intelligence — canonical definitions live in `magistr-core`.
//!
//! This module re-exports the lint, doctor, and suggest capabilities
//! from magistr-core for backward-compatible `crate::lint::*` import paths.

pub use magistr_core::detection::lint::*;
