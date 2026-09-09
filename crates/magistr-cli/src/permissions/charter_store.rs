//! Where a project's charter lives on disk.

use std::path::{Path, PathBuf};

use magistr_engine::policy::Charter;

use crate::error::{MagistrError, Result};

/// The file the reader's [`Charter`] is kept in for a project —
/// `.magistr/charter.json`, beside the roster it sits under and apart
/// from it: the roster is the team's, committed; the charter is one
/// reader's track record on one machine, and ignored by git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharterStore {
    path: PathBuf,
}

impl CharterStore {
    /// The store for the project at `project_dir`.
    #[must_use]
    pub fn at(project_dir: &Path) -> Self {
        Self {
            path: crate::paths::magistr_dir(project_dir).join("charter.json"),
        }
    }

    /// The file itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The charter as saved; an absent file is the empty charter, an
    /// unreadable one an error — never silently a fresh start, which
    /// the next save would make permanent.
    pub fn load(&self) -> Result<Charter> {
        if !self.path.exists() {
            return Ok(Charter::default());
        }
        let text =
            std::fs::read_to_string(&self.path).map_err(|e| MagistrError::io(&self.path, e))?;
        serde_json::from_str(&text)
            .map_err(|e| MagistrError::Config(format!("{}: {e}", self.path.display())))
    }

    /// Write `charter` out, creating `.magistr/` if it is not there.
    pub fn save(&self, charter: &Charter) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
        }
        let text = serde_json::to_string_pretty(charter)
            .map_err(|e| MagistrError::Serialize(e.to_string()))?;
        std::fs::write(&self.path, text).map_err(|e| MagistrError::io(&self.path, e))
    }
}

#[cfg(test)]
#[path = "charter_store.tests.rs"]
mod tests;
