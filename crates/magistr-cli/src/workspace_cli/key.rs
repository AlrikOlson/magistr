//! `magistr workspace key rotate`: replace the per-workspace MRTR
//! confirm-token key, voiding every outstanding confirm token.

use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::{jail::WorkspaceKeyStore, service::WorkspaceOpenError};

use super::fail;

/// Rotate the key of the workspace at `dir`; a workspace that never
/// served has no state directory and is refused with the retained 66.
pub fn run_key_rotate(dir: &Path) -> eyre::Result<()> {
    let state_dir = std::path::absolute(dir)?.join(".magistr/workspace");
    if !state_dir.is_dir() {
        fail(
            WorkspaceOpenError::EXIT_NO_INPUT,
            &format!(
                "not a workspace workspace (no {}) — start the server once first",
                state_dir.display()
            ),
        );
    }
    WorkspaceKeyStore::new(&state_dir).rotate()?;
    eprintln!("magistr workspace: workspace key rotated; outstanding confirm tokens are void");
    Ok(())
}
