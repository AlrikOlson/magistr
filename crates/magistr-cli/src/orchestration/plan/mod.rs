//! YAML-backed plan management — canonical definitions live in `magistr-core`.
//!
//! This module re-exports the plan system from magistr-core
//! for current CLI callers.

pub use magistr_core::plan::*;

mod open_native_reader;
pub use open_native_reader::open_native_reader;
