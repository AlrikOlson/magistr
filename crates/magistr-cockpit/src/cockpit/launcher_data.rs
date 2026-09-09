//! What the launcher's cards show, delivered by one job.

use super::launcher_session::LauncherSession;

/// The launcher's card data: gathered off the loop thread by the entry
/// point (project config, git, the backend PATH scan, the session
/// database) and posted back as one
/// [`JobOutput::Launcher`](crate::job::JobOutput::Launcher), so every
/// card populates after a single job round-trip. Roadmap progress is
/// not here — the Roadmap sub-state already holds the chunks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LauncherData {
    /// The project's name (from the config, or the directory).
    pub project: String,
    /// Whether `.magistr/project.json` exists — without it the project
    /// card points at `magistr init` instead.
    pub has_config: bool,
    /// The stack summary (`Rust / Cargo`).
    pub stack: String,
    /// The git summary (`master, clean` — or `not a git repository`).
    pub git: String,
    /// The backend summary (`ACP (available)` — or `no backend found`).
    pub backend: String,
    /// The most recent sessions for this project, newest first.
    pub sessions: Vec<LauncherSession>,
}
