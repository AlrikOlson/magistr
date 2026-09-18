//! YAML-backed roadmap management — canonical definitions live in `magistr-core`.
//!
//! This module re-exports the roadmap system from magistr-core
//! for current CLI callers.

pub use magistr_core::roadmap::*;

mod admit_native_host;
mod open_native_reader;
mod serve_owner_endpoint;
pub use admit_native_host::admit_native_host;
pub use open_native_reader::open_native_reader;
pub use serve_owner_endpoint::serve_owner_endpoint;
