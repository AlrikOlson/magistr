//! `magistr` with no subcommand: the cockpit's launcher (Home tab).

use std::path::Path;

use color_eyre::eyre;
use magistr_cockpit::cockpit::{LaunchAction, StageTab, Summary};
use magistr_cockpit::job::{JobOutput, Jobs};

use crate::agent;

/// Iterations `l` starts the loop with — the `next-loop` CLI default.
const LOOP_ITERATIONS: u32 = 10;

/// Bare `magistr` (TTY) opens the cockpit on the Home tab — the
/// launcher. The card data (project config, git, the backend PATH scan,
/// the session database) is gathered by one job spawned on the runtime's
/// inbox before the terminal is taken over, so every card populates
/// after a single round-trip. An action button quits the cockpit with a
/// launch ask on the model; it is dispatched here, on the restored
/// terminal — `n` one iteration, `l` the loop, `g` the gates, a session
/// row the cockpit again on that session, continued.
pub async fn cmd_home(env: &crate::env::MagistrEnv) -> eyre::Result<()> {
    let root = std::env::current_dir()?;
    let dir = root.clone();
    let model = super::cockpit::cockpit_model_on(&root, StageTab::Home, move |runtime| {
        let jobs = Jobs::new(runtime.inbox());
        jobs.spawn_job(move || JobOutput::Launcher(Box::new(super::home_data::collect(&dir))));
    })
    .await?;
    match model.launch().cloned() {
        Some(LaunchAction::Next) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto)?;
            let profile = super::run::resolve_permission_profile(None)
                .unwrap_or(crate::permissions::PermissionProfile::Standard);
            super::next::cmd_next_tui(&root, backend, profile, env).await
        }
        Some(LaunchAction::Loop) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto)?;
            super::next::cmd_next_loop_tui(&root, LOOP_ITERATIONS, backend, env).await
        }
        Some(LaunchAction::Gates) => super::validate::cmd_validate(&root).await,
        Some(LaunchAction::Resume(session_id)) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto)?;
            let profile = super::run::resolve_permission_profile(None)
                .unwrap_or(crate::permissions::PermissionProfile::Standard);
            super::run::cmd_run_cockpit(resume_config(&root, session_id, backend), profile).await
        }
        // Birth is the window's flow; a terminal never
        // raises the ask, but were it to, the claim is one iteration on
        // the new folder.
        Some(LaunchAction::Born { dir, .. }) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto)?;
            let profile = super::run::resolve_permission_profile(None)
                .unwrap_or(crate::permissions::PermissionProfile::Standard);
            super::next::cmd_next_tui(std::path::Path::new(&dir), backend, profile, env).await
        }
        None => {
            println!("{}", Summary::of(&model));
            Ok(())
        }
    }
}

/// The agent config a resumed session row asks for: no opening prompt —
/// the cockpit comes up on the continued conversation waiting for the
/// next turn — and `session_id` as the ACP transport's `session/load`.
/// An id the agent no longer holds fails the handshake, which
/// [`cmd_run_cockpit`] reports on stdout rather than panicking.
///
/// [`cmd_run_cockpit`]: super::run::cmd_run_cockpit
fn resume_config(root: &Path, session_id: String, backend: agent::Backend) -> agent::AgentConfig {
    agent::AgentConfig::new("", root)
        .backend(backend)
        .resume(session_id)
        .with_project_context(root)
}

#[cfg(test)]
#[path = "home.tests.rs"]
mod tests;
