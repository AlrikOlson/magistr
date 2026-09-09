//! Spec file parsing for the workflow engine.
//!
//! Pure parsing logic lives in `magistr-engine`. This module re-exports
//! those types and adds IO wrappers for reading spec files from disk.

use std::path::Path;

use crate::error::Result;
use crate::workflow::spec_parse::parse_spec;

// Re-export pure types from engine.
pub use magistr_engine::spec::SpecSummary;

/// Build a [`SpecSummary`] from the YAML store if available, falling back to markdown parsing.
///
/// This is the preferred entry point for workflow code. It reads directly
/// from `.magistr/roadmap.yaml` (via [`RoadmapIndex::to_spec_summary`]) so the
/// numbers always match the YAML store. Falls back to markdown for
/// non-magistr projects or when the YAML file doesn't exist.
pub fn spec_summary_from_project(project_dir: &Path, spec_path: &Path) -> Result<SpecSummary> {
    let yaml_path = crate::roadmap::yaml_path(project_dir);
    if let Ok(store) = crate::roadmap::YamlStore::load(&yaml_path) {
        return Ok(store.index().to_spec_summary());
    }
    // Fallback for non-magistr projects or missing YAML
    parse_spec(spec_path)
}

#[cfg(test)]
#[path = "spec.tests.rs"]
mod tests;
