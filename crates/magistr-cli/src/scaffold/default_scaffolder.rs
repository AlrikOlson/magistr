//! Filesystem-based [`Scaffolder`] implementation.
//!
//! Delegates to the existing `generate()` and `preview()` free functions.
//! This is the production implementation.

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

use super::ChangePlan;
use super::scaffolder::Scaffolder;

/// Generates scaffold files by writing to the filesystem.
///
/// This is the default production implementation of [`Scaffolder`].
/// It delegates to the existing template-based generation pipeline.
pub struct DefaultScaffolder;

impl Scaffolder for DefaultScaffolder {
    fn generate(
        &self,
        dir: &Path,
        template_vars: &HashMap<String, String>,
        generated_overrides: &HashMap<String, String>,
    ) -> Result<Vec<String>> {
        super::generate(dir, template_vars, generated_overrides)
    }

    fn preview(
        &self,
        dir: &Path,
        template_vars: &HashMap<String, String>,
        generated_overrides: &HashMap<String, String>,
    ) -> Result<ChangePlan> {
        super::preview(dir, template_vars, generated_overrides)
    }
}

#[cfg(test)]
#[path = "default_scaffolder.tests.rs"]
mod tests;
