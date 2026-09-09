//! Quality gate command classification — canonical definition lives in `magistr-core`.

pub use magistr_core::tools::gate_patterns::*;

#[cfg(test)]
#[path = "gate_patterns.tests.rs"]
mod tests;
