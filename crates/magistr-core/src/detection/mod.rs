//! Detection domain — stack detection, validation, and linting.
//!
//! Contains the generic `Detection<T>` wrapper, stack detection logic,
//! and CLAUDE.md linting/doctor capabilities.

pub mod detect;
mod generic;
pub mod lint;

pub use generic::Detection;
