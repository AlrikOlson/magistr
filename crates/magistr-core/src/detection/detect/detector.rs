//! Trait abstraction for project stack detection.
//!
//! Enables dependency injection and testability by decoupling detection
//! logic from filesystem access.

use std::path::Path;

use crate::error::Result;
use crate::types::Stack;

/// Detects the project stack (language, framework, package manager, etc.)
/// from a project directory.
///
/// The default implementation ([`FileSystemDetector`](super::FileSystemDetector))
/// reads the filesystem directly. Alternative implementations enable testing
/// without touching the filesystem.
pub trait Detector: Send + Sync {
    /// Detect the full project stack from a directory.
    fn detect_stack(&self, dir: &Path) -> Result<Stack>;
}

#[cfg(test)]
#[path = "detector.tests.rs"]
mod tests;
