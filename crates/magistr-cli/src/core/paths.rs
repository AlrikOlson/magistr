//! Centralized path definitions — canonical definitions live in `magistr-core`.

pub use std::path::{Path, PathBuf};

pub use magistr_core::paths::*;

#[cfg(test)]
#[path = "paths.tests.rs"]
mod tests;
