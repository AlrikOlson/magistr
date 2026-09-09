//! Trait abstraction for scaffold generation.
//!
//! Enables dependency injection and testability by decoupling scaffold
//! logic from filesystem writes and template processing.

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

use super::ScaffoldPreview;

/// Generates scaffold files for a project.
///
/// The default implementation ([`DefaultScaffolder`](super::DefaultScaffolder))
/// uses the template engine and writes files to disk. Alternative implementations
/// enable testing without filesystem side effects.
pub trait Scaffolder: Send + Sync {
    /// Generate the full scaffold in `dir`.
    ///
    /// Returns paths of files that were written (created or updated).
    fn generate(
        &self,
        dir: &Path,
        template_vars: &HashMap<String, String>,
        generated_overrides: &HashMap<String, String>,
    ) -> Result<Vec<String>>;

    /// Preview what `generate()` would do without writing any files.
    fn preview(
        &self,
        dir: &Path,
        template_vars: &HashMap<String, String>,
        generated_overrides: &HashMap<String, String>,
    ) -> Result<ScaffoldPreview>;
}

#[cfg(test)]
#[path = "scaffolder.tests.rs"]
mod tests;
