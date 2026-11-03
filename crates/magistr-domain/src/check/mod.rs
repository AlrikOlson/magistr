//! Check domain types — status tracking for check/test/lint checks.
//!
//! Defines the abstract types for check status. The concrete check
//! runner (process spawning, caching, parallelism) lives in `magistr-cli`.

pub mod check_state;
pub mod standard_check_kind;
pub mod standard_check_outcome;

pub use check_state::CheckState;
pub use standard_check_kind::StandardCheckKind;
pub use standard_check_outcome::StandardCheckOutcome;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
