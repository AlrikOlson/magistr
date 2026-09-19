//! Plain workflow session bound to the selected project.

use crate::agent;
use crate::error::ResultExt;
use crate::permissions::PermissionProfile;
use color_eyre::eyre;
use magistr_application::effects::SourceRoute;

/// Run a workflow prompt under its selected project's shared permission
/// policy; `source` is the host's route for the session's authorized source
/// writes (the workspace it admitted for this run or loop, or its refusal).
/// `launched` first checkpoints with no handle, inside the transport before
/// spawn, then records the provider's process id when connected. A failed
/// first checkpoint prevents spawn; a failed handle checkpoint ends the session.
pub async fn cmd_run_workflow(
    prompt: &str,
    project_dir: &std::path::Path,
    backend: agent::Backend,
    profile: PermissionProfile,
    source: SourceRoute,
    mut launched: impl FnMut(Option<u32>) -> eyre::Result<()>,
) -> eyre::Result<()> {
    let config = agent::AgentConfig::new(prompt, project_dir)
        .backend(backend)
        .permission_profile(profile)
        .workflow_permissions(true)
        .allowed_tools(agent::ToolPermissions::for_workflow().into_strings())
        .with_project_context(project_dir);
    // The inputs the session was composed with: the drain finds the ledger
    // and the actor through the same capture.
    let inputs = config.inputs().clone();
    let mut session = agent::session::AgentSession::start_checkpointed(config, source, || {
        launched(None).map_err(|error| crate::error::MagistrError::Workflow(error.to_string()))
    })
    .await
    .suggest()?;
    if let Err(error) = launched(session.provider_pid()) {
        let _ = session.terminate_session().await;
        return Err(error);
    }
    let model = session.resolved_model().map(str::to_owned);
    let outcome =
        super::drain_session_interactive(&mut session, project_dir, &inputs, model.as_deref())
            .await;
    let cleanup = session.terminate_session().await.suggest();
    outcome.and(cleanup)
}
