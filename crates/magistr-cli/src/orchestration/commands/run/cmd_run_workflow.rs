//! Plain workflow session bound to the selected project.

use crate::agent;
use crate::error::ResultExt;
use crate::permissions::PermissionProfile;
use color_eyre::eyre;

/// Run a workflow prompt under its selected project's shared permission policy.
pub async fn cmd_run_workflow(
    prompt: &str,
    project_dir: &std::path::Path,
    backend: agent::Backend,
    profile: PermissionProfile,
) -> eyre::Result<()> {
    let model = crate::permissions::governed_model(project_dir, None)?;
    let mut config = agent::AgentConfig::new(prompt, project_dir)
        .backend(backend)
        .permission_profile(profile)
        .workflow_permissions(true)
        .allowed_tools(agent::ToolPermissions::for_workflow().into_strings())
        .with_project_context(project_dir);
    config.model = model.clone();
    let mut session = agent::session::AgentSession::start(config)
        .await
        .suggest()?;
    super::drain_session_interactive(&mut session, project_dir, model.as_deref()).await
}
