use std::path::Path;

use crate::error::{MagistrError, Result};

use super::emitters;
use super::manifest::ScaffoldManifest;
use super::owned_removal::OwnedRemoval;

/// Remove unchanged, recorded scaffold files from shared agent directories.
///
/// A known filename is not ownership evidence: customized and unrecorded files
/// survive, as do non-regular entries and paths traversing symlinks. Only empty
/// parents of removed files are pruned. CLAUDE.md and ROADMAP.md remain user-owned.
///
/// This is still the explicit reset operation: it removes the entire .magistr
/// and legacy .jig namespaces, and managed MCP/gitignore entries. It is not a
/// state-preserving upgrade; initialization never calls it.
pub fn remove_scaffold(dir: &Path) -> Result<()> {
    // Capture ownership once, before removing the namespace holding the manifest.
    let manifest = ScaffoldManifest::load(dir);
    super::CodexSetup::remove_shared(dir, &manifest)?;
    let mut removal = OwnedRemoval::new(dir, &manifest);
    let codex_paths = super::CodexSetup::removal_paths(&manifest);
    for path in manifest.files.keys() {
        // Include recorded files retired by a newer binary, but never let an
        // arbitrary manifest entry claim a root project document or source file.
        if Path::new(path).starts_with(".claude") || codex_paths.contains(path) {
            removal.remove(path)?;
        }
    }
    emitters::remove_emitter_files(&mut removal)?;
    for path in ["run-autonomous.sh", "format-stream.sh"] {
        removal.remove(path)?;
    }
    removal.prune();

    let magistr_dir = crate::paths::magistr_dir(dir);
    if magistr_dir.exists() {
        std::fs::remove_dir_all(&magistr_dir).map_err(|e| MagistrError::io(&magistr_dir, e))?;
    }
    let legacy_dir = dir.join(".jig");
    if legacy_dir.exists() {
        std::fs::remove_dir_all(&legacy_dir).map_err(|e| MagistrError::io(&legacy_dir, e))?;
    }
    super::remove_jig_mcp_entries(dir)?;
    super::remove_magistr_gitignore_entries(dir)?;
    Ok(())
}

#[cfg(test)]
#[path = "remove_scaffold.tests.rs"]
mod tests;
