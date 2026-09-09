//! Centralized environment configuration — canonical definition lives in `magistr-core`.

pub use magistr_core::env::*;

#[cfg(test)]
#[path = "env.tests.rs"]
mod tests;
