use crate::agent;
use color_eyre::eyre;

/// Run the roadmap loop in the terminal cockpit.
pub async fn cmd_next_loop_tui(
    dir: &std::path::Path,
    max_iterations: u32,
    scope: Option<&magistr_cockpit::cockpit::LoopScope>,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
    _env: &crate::env::MagistrEnv,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    super::cmd_next_loop_cockpit(dir, max_iterations, scope, backend, profile, inputs).await
}
