//! Detection domain — stack detection, validation, and linting.
//!
//! Detects project language, framework, package manager, test framework,
//! CI provider, and workspace type. Also provides configuration validation
//! and project-level linting.

pub mod detect;
pub mod lint;
pub mod validate;
