use crate::agent;
use crate::permissions::PermissionProfile;
use color_eyre::eyre;

/// Launch the terminal cockpit with the selected agent and permission profile.
pub async fn cmd_run_tui(
    prompt: &str,
    resume: Option<&str>,
    continue_conversation: bool,
    model: Option<&str>,
    backend: agent::Backend,
    profile: PermissionProfile,
    _env: &crate::env::MagistrEnv,
) -> eyre::Result<()> {
    let cwd = std::env::current_dir()?;
    let config = agent::AgentConfig::new(prompt, &cwd)
        .backend(backend)
        .with_run_args(model, resume, continue_conversation)
        .with_project_context(&cwd);

    super::cmd_run_cockpit(config, profile).await
}
