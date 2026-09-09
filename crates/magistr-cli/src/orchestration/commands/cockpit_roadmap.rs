//! `magistr roadmap` (and the launcher's Roadmap action): the cockpit
//! opened on its Roadmap tab — the legacy roadmap TUI was deleted with

use std::path::Path;

use color_eyre::eyre;
use magistr_cockpit::cockpit::StageTab;

/// [`cmd_cockpit`](super::cockpit::cmd_cockpit) opened on the Roadmap
/// tab.
pub async fn cmd_cockpit_roadmap(root: &Path) -> eyre::Result<()> {
    super::cockpit::cmd_cockpit_on(root, StageTab::Roadmap).await
}
