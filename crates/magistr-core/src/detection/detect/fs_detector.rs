//! Filesystem-based [`Detector`] implementation.
//!
//! Reads manifest files, config files, and directory structure to detect
//! the project stack. This is the production implementation.

use std::path::Path;

use crate::error::Result;
use crate::types::Stack;

use super::detector::Detector;

/// Detects the project stack by reading the filesystem directly.
///
/// This is the default production implementation of [`Detector`].
/// It delegates to the existing detection functions in sibling modules
/// (language, framework, package_manager, etc.).
pub struct FileSystemDetector;

impl Detector for FileSystemDetector {
    fn detect_stack(&self, dir: &Path) -> Result<Stack> {
        super::detect_stack(dir)
    }
}

#[cfg(test)]
#[path = "fs_detector.tests.rs"]
mod tests;
