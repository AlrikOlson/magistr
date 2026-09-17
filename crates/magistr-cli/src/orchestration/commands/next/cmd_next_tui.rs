use crate::agent;
use crate::permissions::PermissionProfile;
use color_eyre::eyre;

/// Launch the terminal cockpit for `magistr next`.
///
/// The workflow entry point supplies its own permission policy.
pub async fn cmd_next_tui(
    dir: &std::path::Path,
    backend: agent::Backend,
    profile: PermissionProfile,
    _env: &crate::env::MagistrEnv,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    super::cmd_next_cockpit(dir, backend, profile, inputs).await
}
