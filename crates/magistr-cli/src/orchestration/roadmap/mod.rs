//! YAML-backed roadmap management — canonical definitions live in `magistr-core`.
//!
//! This module re-exports the roadmap system from magistr-core
//! for backward-compatible `crate::roadmap::*` import paths.

pub use magistr_core::roadmap::*;

mod open_native_engines;
pub use open_native_engines::open_native_engines;
