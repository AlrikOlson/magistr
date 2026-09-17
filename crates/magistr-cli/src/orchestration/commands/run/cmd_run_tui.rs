use crate::agent;
use crate::permissions::PermissionProfile;
use color_eyre::eyre;

/// Launch the terminal cockpit with the selected agent and permission profile.
pub async fn cmd_run_tui(
    request: super::RunRequest<'_>,
    backend: agent::Backend,
    profile: PermissionProfile,
    _env: &crate::env::MagistrEnv,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let cwd = inputs.project_root().to_path_buf();
    let config = request
        .apply(agent::AgentConfig::new(request.prompt, &cwd).with_inputs(inputs))
        .backend(backend)
        .with_project_context(&cwd);

    super::cmd_run_cockpit(config, profile).await
}
