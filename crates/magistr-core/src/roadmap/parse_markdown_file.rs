//! File-based markdown parsing entry point.

use std::path::Path;

use crate::error::{MagistrError, Result};

use super::schema::Roadmap;

/// Parse a ROADMAP.md file into a [`Roadmap`] struct.
pub fn parse_markdown_file(path: &Path) -> Result<Roadmap> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| MagistrError::Config(format!("failed to read {}: {e}", path.display())))?;
    super::migrate::parse_markdown(&content)
}
