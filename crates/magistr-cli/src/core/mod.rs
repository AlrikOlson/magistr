//! Core domain types, error handling, paths, and configuration.
//!
//! This module contains foundational types that all other modules depend on.
//! No reverse dependencies: `core` does not import from `detection`, `agent`,
//! or `orchestration`.

pub mod config;
pub mod env;
pub mod error;
pub mod paths;
pub mod types;
