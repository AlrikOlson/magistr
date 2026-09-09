//! Core domain types — canonical definitions live in `magistr-core`.
//!
//! All types are re-exported here so existing `crate::types::*` imports
//! continue to work without changes.

pub use magistr_core::types::*;

// Test-hosting shim — keeps preset_layer tests compiling in the CLI crate.
#[cfg(test)]
mod preset_layer;
