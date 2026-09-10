//! `magistr run` on the mission-control cockpit (`crates/magistr-tui`):
//! the agent's session on the stage, the prompt line under it, permission
//! requests as a strip above the prompt.

use std::time::Duration;

use color_eyre::eyre::{self, Context as _};
use magistr_cockpit::agent::spawn_session;
use magistr_cockpit::cockpit::{Model, Summary};
use magistr_cockpit::engine::{EngineClient, NativeEnginePort, StoreSeed};
use magistr_cockpit::theme::Theme;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RealTerminal, Terminal as _};

use crate::agent;
use crate::agent::cockpit_session::CockpitSession;
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
/// the think-and-ship engines open, then the agent is spawned and its
/// handshake completed — a store that cannot be opened or an agent that
/// cannot start is printed on stdout and nothing else happens. Only then
/// is the terminal taken over, the feed and the session attached, and
/// the loop run; leaving restores the terminal, the session ends the
/// agent, and the summary lands in the scrollback.
pub async fn cmd_run_cockpit(
    config: agent::AgentConfig,
    profile: PermissionProfile,
) -> eyre::Result<()> {
    let root = config.cwd.clone();
    let engines = match EngineClient::open(&root) {
        Ok(engines) => engines,
        Err(err) => {
            println!(
                "magistr: cannot open the think-and-ship store for {}: {err}",
                root.display()
            );
            return Err(eyre::eyre!("the cockpit did not start"));
        }
    };
    let spec = config.permission_profile(profile).resolve()?;
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
        Some(Box::new(NativeEnginePort::new(engines.engines().clone()))),
        Some(Box::new(SpinePort::at(
            session_engine(&root, engines.engines().clone()),
            root.clone(),
        ))),
    );
    let model = runtime
        .run(super::seed_supervision(
            Model::new(size)
                .with_theme(theme)
                .with_title(title)
                .with_debug(debug)
                .with_store_seed(&StoreSeed::read(engines.engines()))
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
    let model = model.context("the cockpit failed")?;
    println!("{}", Summary::of(&model));
    Ok(())
}

#[cfg(test)]
#[path = "cmd_run_cockpit.tests.rs"]
mod tests;
