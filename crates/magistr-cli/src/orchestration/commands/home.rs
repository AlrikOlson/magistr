//! Bare `magistr`: Roadmap with host actions in the footer and palette.

use std::path::Path;

use color_eyre::eyre;
use magistr_cockpit::cockpit::{LaunchAction, StageTab, Summary};
use magistr_cockpit::job::JobOutput;
use magistr_cockpit_runtime::Jobs;

use crate::agent;

/// Iterations `l` starts the loop with — the `next-loop` CLI default.
const LOOP_ITERATIONS: u32 = 10;

/// Bare `magistr` opens Roadmap. One background job supplies project and
/// session data to the palette and enables the host command footer.
/// A launch action quits the cockpit and is dispatched on the restored
/// terminal: Alt-N starts one iteration, Alt-L a loop, Alt-G the gates.
/// Palette session rows resume the selected conversation.
pub async fn cmd_home(
    env: &crate::env::MagistrEnv,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let root = inputs.project_root().to_path_buf();
    let dir = root.clone();
    let captured = inputs.clone();
    let model = super::cockpit::cockpit_model_on(&root, StageTab::Turn, move |runtime| {
        let jobs = Jobs::new(runtime.inbox());
        jobs.spawn_job(move || {
            JobOutput::Launcher(Box::new(super::home_data::collect(&dir, &captured)))
        });
    })
    .await?;
    match model.launch().cloned() {
        Some(LaunchAction::Next) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto, inputs)?;
            let profile = super::run::resolve_permission_profile(&root, None)
                .unwrap_or(crate::permissions::PermissionProfile::Standard);
            super::next::cmd_next_tui(&root, backend, profile, env, inputs).await
        }
        Some(LaunchAction::Loop(scope)) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto, inputs)?;
            let profile = super::run::resolve_permission_profile(&root, None)?;
            super::next::cmd_next_loop_tui(
                &root,
                LOOP_ITERATIONS,
                Some(&scope),
                backend,
                profile,
                env,
                inputs,
            )
            .await
        }
        Some(LaunchAction::Gates) => super::validate::cmd_validate(&root).await,
        Some(LaunchAction::Resume(session_id)) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto, inputs)?;
            let profile = super::run::resolve_permission_profile(&root, None)
                .unwrap_or(crate::permissions::PermissionProfile::Standard);
            super::run::cmd_run_cockpit(resume_config(inputs, session_id, backend), profile).await
        }
        // Birth is the window's flow; a terminal never
        // raises the ask, but were it to, the claim is one iteration on
        // the new folder.
        Some(LaunchAction::Born { dir, .. }) => {
            let backend = super::resolve_backend(crate::cli::BackendFlag::Auto, inputs)?;
            let profile = super::run::resolve_permission_profile(std::path::Path::new(&dir), None)
                .unwrap_or(crate::permissions::PermissionProfile::Standard);
            super::next::cmd_next_tui(
                std::path::Path::new(&dir),
                backend,
                profile,
                env,
                &inputs.for_project(&dir),
            )
            .await
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
fn resume_config(
    inputs: &crate::config::inputs::ProjectInputs,
    session_id: String,
    backend: agent::Backend,
) -> agent::AgentConfig {
    let root: &Path = inputs.project_root();
    agent::AgentConfig::new("", root)
        .with_inputs(inputs)
        .backend(backend)
        .resume(session_id)
        .with_project_context(root)
}

#[cfg(test)]
#[path = "home.tests.rs"]
mod tests;
