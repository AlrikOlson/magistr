//! Raw spec file reading.

use std::path::Path;

use crate::error::{MagistrError, Result};

/// Read the raw content of a spec file.
pub fn read_spec(spec_path: &Path) -> Result<String> {
    std::fs::read_to_string(spec_path).map_err(|e| MagistrError::io(spec_path, e))
}
