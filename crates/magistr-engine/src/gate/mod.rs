//! Quality gate domain types — status tracking for check/test/lint gates.
//!
//! Defines the abstract types for quality gate status. The concrete gate
//! runner (process spawning, caching, parallelism) lives in `magistr-cli`.

pub mod gate_kind;
pub mod gate_outcome;
pub mod gate_status;

pub use gate_kind::GateKind;
pub use gate_outcome::GateOutcome;
pub use gate_status::GateStatus;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
