//! `magistr run` on the mission-control cockpit (`crates/magistr-tui`):
//! the agent's session on the stage, the prompt line under it, permission
//! requests as a strip above the prompt.

use std::time::Duration;

use color_eyre::eyre::{self, Context as _};
use magistr_cockpit::agent::spawn_session;
use magistr_cockpit::cockpit::{Model, Summary};
use magistr_cockpit::engine::{EngineClient, StoreSeed};
use magistr_cockpit::theme::Theme;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RealTerminal, Terminal as _};

use crate::agent;
use crate::agent::cockpit_session::CockpitSession;
use crate::application::HostedWorkspace;
use crate::orchestration::ledger::{SpinePort, session_engine};
use crate::permissions::PermissionProfile;

/// How long the session gets to end the agent once the cockpit is gone.
const SESSION_GRACE: Duration = Duration::from_secs(5);

/// Run `config`'s prompt as the opening turn of a live session on the
/// cockpit and keep the conversation going until the user quits.
/// Permission requests are put to `profile`'s [`CockpitPolicy`] before
/// they reach the strip — protected paths are denied, the profile's
/// allows answered without asking.
///
/// [`CockpitPolicy`]: crate::permissions::CockpitPolicy
///
/// Everything that can fail loudly happens on the normal terminal first:
/// this process is admitted as the project's host and the think-and-ship
/// engines open — or, when a loop or another `run` already holds the host,
/// it attaches to that host's owner endpoint and acts through its writer —
/// and the workspace is admitted in this process (its owner endpoint
/// served, so the agent's bridge and the cockpit's port attach to it and
/// the session's authorized writes run through its pipeline) or, when
/// another owner holds it, left to that owner with the writes going direct,
/// then the agent is spawned and its handshake completed. A store that
/// cannot be opened, a held host with no endpoint to attach to, or an agent
/// that cannot start is printed on stdout and nothing else happens. Only
/// then is the terminal taken over, the feed and the session attached, and
/// the loop run; leaving restores the terminal, the session ends the
/// agent, and the summary lands in the scrollback. A host serves its own
/// owner endpoint for the run, so the next cockpit on the project attaches
/// the same way.
pub async fn cmd_run_cockpit(
    config: agent::AgentConfig,
    profile: PermissionProfile,
) -> eyre::Result<()> {
    let root = config.cwd.clone();
    let engines = match EngineClient::admit_or_attach_config(magistr_core::evidence::writer_config(
        &root,
        |key| std::env::var(key).ok(),
    )) {
        Ok(engines) => engines,
        Err(err) => {
            println!(
                "magistr: cannot open the think-and-ship store for {}: {err}",
                root.display()
            );
            return Err(eyre::eyre!("the cockpit did not start"));
        }
    };
    let engine_port = engines
        .port()
        .ok_or_else(|| eyre::eyre!("the cockpit run was opened without a host or an owner"))?;
    // Dropped after the runtime and before the engines: the endpoint stops
    // taking clients before this host releases the project.
    let owner_endpoint = match engines.serve_owner().await {
        Ok(endpoint) => endpoint,
        Err(err) => {
            println!("magistr: {err}");
            return Err(eyre::eyre!("the cockpit did not start"));
        }
    };
    let workspace = admit_workspace(&root).await;
    let writer = match workspace.as_ref().map(HostedWorkspace::source_writer) {
        Some(Ok(writer)) => Some(writer),
        Some(Err(err)) => {
            println!("magistr: workspace session refused ({err}); agent writes go direct");
            None
        }
        None => None,
    };
    let spec = config
        .permission_profile(profile)
        .resolve()?
        .with_source_writer(writer);
    let root = spec.cwd().to_path_buf();
    let profile = spec.permission_profile();
    let workflow = spec.workflow_permissions();
    let (title, debug) =
        crate::orchestration::commands::chrome::chrome(&root, Some("acp"), spec.model());
    let session_handle = match CockpitSession::start(spec).await {
        Ok(session) => session,
        Err(err) => {
            println!("magistr: cannot start the agent: {err}");
            return Err(eyre::eyre!("the cockpit did not start"));
        }
    };
    let theme = Theme::from_os();
    let terminal =
        RealTerminal::stdio().context("the cockpit needs a terminal on stdin and stdout")?;
    let size = terminal.size()?;
    let mut runtime = Runtime::new(terminal, Options::from_os());
    let commands = runtime
        .commands()
        .ok_or_else(|| eyre::eyre!("the runtime's commands were already taken"))?;
    let feed = engines.subscribe(runtime.inbox());
    // The safety net under the socket: what the agent's own
    // `magistr mcp-serve` records reaches the tabs even with no feed.
    let reseed = engines.reseed(runtime.inbox());
    // The workbench port is the project's own workbench-mcp, spoken to
    // over its stdio; the server is only spawned once something is asked
    // of it. The engine port is the same in-process engines the rail
    // seed reads.
    let mut session = spawn_session(
        Box::new(session_handle),
        runtime.inbox(),
        commands,
        crate::orchestration::commands::workbench_port::workbench_port(&root),
        Some(engine_port),
        Some(Box::new(SpinePort::at(
            session_engine(&root, engines.reader().clone()),
            root.clone(),
        ))),
    );
    let model = runtime
        .run(super::seed_supervision(
            Model::new(size)
                .with_theme(theme)
                .with_title(title)
                .with_debug(debug)
                .with_store_seed(&StoreSeed::read(engines.reader()))
                .with_workspace_files(
                    crate::orchestration::commands::workspace_files::workspace_files(&root),
                ),
            &root,
            profile,
            workflow,
        ))
        .await;
    feed.abort();
    reseed.abort();
    drop(owner_endpoint);
    // Dropping a JoinHandle detaches its task. Keep ownership across the
    // deadline and await an aborted relay so its supervisor is released.
    match tokio::time::timeout(SESSION_GRACE, &mut session).await {
        Ok(result) => result
            .context("the session relay failed")?
            .map_err(|error| eyre::eyre!("session cleanup failed: {error}"))?,
        Err(_) => {
            session.abort();
            let _ = session.await;
            return Err(eyre::eyre!(
                "session cleanup exceeded its deadline; the owner was aborted"
            ));
        }
    }
    if let Some(workspace) = workspace
        && let Err(err) = workspace.shutdown().await
    {
        println!("magistr: workspace shutdown: {err}");
    }
    let model = model.context("the cockpit failed")?;
    println!("{}", Summary::of(&model));
    Ok(())
}

/// Admit the run's workspace in this process, or say on the normal
/// terminal why the session's writes will go direct: a live owner
/// elsewhere, or a root this binary cannot admit.
pub(crate) async fn admit_workspace(root: &std::path::Path) -> Option<HostedWorkspace> {
    match HostedWorkspace::admit(root).await {
        Ok(Some(workspace)) => {
            println!(
                "magistr: workspace admitted (generation {}); agent writes run through its pipeline",
                workspace.generation()
            );
            Some(workspace)
        }
        Ok(None) => {
            println!("magistr: another owner holds the workspace; agent writes go direct");
            None
        }
        Err(err) => {
            println!("magistr: workspace not admitted ({err}); agent writes go direct");
            None
        }
    }
}

#[cfg(test)]
#[path = "cmd_run_cockpit.tests.rs"]
mod tests;
