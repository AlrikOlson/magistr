use super::*;
use crate::agent;
use crate::error::ResultExt;
use crate::utils::format as fmt;
use color_eyre::eyre;

pub async fn cmd_run(
    prompt: &str,
    resume: Option<&str>,
    continue_conversation: bool,
    model: Option<&str>,
    backend: agent::Backend,
    profile: crate::permissions::PermissionProfile,
) -> eyre::Result<()> {
    let cwd = std::env::current_dir()?;
    let config = agent::AgentConfig::new(prompt, &cwd)
        .backend(backend)
        .permission_profile(profile)
        .with_run_args(model, resume, continue_conversation)
        .with_project_context(&cwd);

    let mut session = agent::session::AgentSession::start(config)
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
    outcome.and(cleanup)
}
