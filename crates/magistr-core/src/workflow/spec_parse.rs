//! Spec file parsing — read and parse a spec file into a summary.

use std::path::Path;

use crate::error::{MagistrError, Result};
use magistr_engine::spec::SpecSummary;

/// Read and parse a spec file, returning a summary of item status.
pub fn parse_spec(spec_path: &Path) -> Result<SpecSummary> {
    let content = std::fs::read_to_string(spec_path).map_err(|e| MagistrError::io(spec_path, e))?;
    Ok(SpecSummary::parse(&content))
}
