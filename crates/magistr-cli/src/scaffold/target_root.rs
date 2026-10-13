//! Setup writes project settings through their shared owner and other files in the selected copy.
use std::path::{Path, PathBuf};

use crate::error::{MagistrError, Result};

pub(super) fn target_root(dir: &Path, relative: &str) -> Result<PathBuf> {
    if relative == ".magistr/project.json" {
        let path = crate::paths::project_json(dir)?;
        path.parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                MagistrError::Config("Cannot locate the project settings directory.".into())
            })
    } else {
        Ok(dir.to_path_buf())
    }
}

#[cfg(test)]
#[path = "target_root.tests.rs"]
mod tests;
