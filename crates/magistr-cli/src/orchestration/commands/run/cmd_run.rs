use super::*;
use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub async fn cmd_run(
    request: RunRequest<'_>,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    let cwd = inputs.project_root().to_path_buf();
    let prompt = request.prompt;
    let config = request
        .apply(agent::AgentConfig::new(prompt, &cwd).with_inputs(inputs))
        .backend(backend)
        .permission_profile(profile)
        .with_project_context(&cwd);

    // The one workspace composition every source-writing host shares:
    // admitted, the session's writes run through the pipeline; refused,
    // they are refused — never written directly.
    let workspace = admit_workspace(inputs, |line| eprintln!("{line}")).await;
    let mut session = agent::session::AgentSession::start(config, workspace.source_route())
        .await
        .suggest()?;

    let model_name = session.resolved_model().map(str::to_owned);
    if let Some(model_name) = &model_name {
        eprintln!(
            "{} Running agent with prompt: {prompt}  {}",
            fmt::cyan("→"),
            fmt::dim(&format!("(model: {model_name})"))
        );
    } else {
        eprintln!(
            "{}",
            fmt::info(&format!("Running agent with prompt: {prompt}"))
        );
    }
    eprintln!();

    let outcome = drain_session_interactive(&mut session, &cwd, model_name.as_deref()).await;
    let cleanup = session.terminate_session().await.suggest();
    drop(session);
    if let Err(err) = workspace.shutdown().await {
        eprintln!("magistr: workspace shutdown: {err}");
    }
    outcome.and(cleanup)
}
