use crate::agent;
use color_eyre::eyre;

/// Run the roadmap loop in the terminal cockpit.
pub async fn cmd_next_loop_tui(
    dir: &std::path::Path,
    max_iterations: u32,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
    _env: &crate::env::MagistrEnv,
) -> eyre::Result<()> {
    super::cmd_next_loop_cockpit(dir, max_iterations, backend, profile).await
}
