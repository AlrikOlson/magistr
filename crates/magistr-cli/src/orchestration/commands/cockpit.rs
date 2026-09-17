//! The mission-control cockpit (`crates/magistr-tui`) on the real
//! terminal — the shared entry point behind bare `magistr` and
//! `magistr roadmap`.

use std::path::Path;
use std::time::Duration;

use crate::agent::AgentConfig;
use crate::orchestration::interactive_session::InteractiveSession;
use crate::orchestration::loop_driver::Connector;
use magistr_application::effects::{SourceRefusal, SourceRoute};

use color_eyre::eyre::{self, Context as _};
use magistr_cockpit::cockpit::{Model, StageTab, Summary};
use magistr_cockpit::engine::EngineClient;
use magistr_cockpit::theme::Theme;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RealTerminal, Terminal as _};

/// Open the cockpit for the project at `root` and run it until it quits
/// (ctrl-D or ctrl-C).
///
/// The think-and-ship engines open first, on the normal terminal: a store
/// that cannot be opened is printed on stdout and nothing else happens.
/// Only then is the terminal taken over — raw mode, the alternate screen,
/// mouse capture, the kitty keyboard protocol — the live feed attached and
/// the loop run. Leaving restores all of it in reverse and joins the
/// input thread; the session's summary is printed once the terminal is
/// handed back, so it lands in the scrollback.
pub async fn cmd_cockpit(root: &Path) -> eyre::Result<()> {
    cmd_cockpit_on(root, StageTab::Turn).await
}

/// Run the agent-less cockpit on `tab` and hand back the final model —
/// what [`cmd_cockpit_on`] prints a summary of, and what the launcher
/// ([`cmd_home`](super::home::cmd_home)) reads its exit ask from.
/// `before_run` sees the runtime after the engines subscribed and before
/// the terminal is taken over — where the launcher spawns its data job.
pub(crate) async fn cockpit_model_on(
    root: &Path,
    tab: StageTab,
    before_run: impl FnOnce(&Runtime<RealTerminal>),
) -> eyre::Result<Model> {
    let engines = match EngineClient::observe(root) {
        Ok(engines) => engines,
        Err(err) => {
            println!(
                "magistr: cannot open saved project data for {}: {err}",
                root.display()
            );
            return Err(eyre::eyre!("Magistr could not start this session"));
        }
    };
    let theme = Theme::from_os();
    let (title, debug) = super::chrome::chrome(root, None, None);
    let terminal = RealTerminal::stdio().context("open Magistr in a terminal to use this view")?;
    let size = terminal.size()?;
    let mut runtime = Runtime::new(terminal, Options::from_os());
    let commands = runtime
        .commands()
        .ok_or_else(|| eyre::eyre!("Magistr could not connect the session controls"))?;
    let mut session = InteractiveSession::new(
        AgentConfig::new("", root).with_project_context(root),
        Connector::live(SourceRoute::Refused(SourceRefusal::NoAuthority)),
        runtime.inbox(),
        Some(engines.clone()),
    )
    .spawn(commands);
    let feed = engines.subscribe(runtime.inbox());
    // The store as the model first sees it; the re-seed measures every
    // later read against it.
    let seed = engines.reader().snapshot();
    // No loop driver here: without the broadcast socket the store is
    // only re-read by this timer.
    let reseed = engines.reseed(runtime.inbox(), seed.cursor);
    before_run(&runtime);
    let model = runtime
        .run(
            Model::new(size)
                .with_theme(theme)
                .with_title(title)
                .with_debug(debug)
                .with_tab(tab)
                .with_snapshot(&seed)
                .with_workspace_files(super::workspace_files::workspace_files(root)),
        )
        .await;
    feed.abort();
    reseed.abort();
    if tokio::time::timeout(Duration::from_secs(5), &mut session)
        .await
        .is_err()
    {
        session.abort();
        let _ = session.await;
    }
    model.context("the session ended unexpectedly")
}

/// The cockpit without an agent session, starting on `tab` — what
/// [`cmd_cockpit`] and the Roadmap entry
/// ([`cmd_cockpit_roadmap`](super::cmd_cockpit_roadmap)) share.
pub(crate) async fn cmd_cockpit_on(root: &Path, tab: StageTab) -> eyre::Result<()> {
    let model = cockpit_model_on(root, tab, |_| {}).await?;
    println!("{}", Summary::of(&model));
    Ok(())
}

#[cfg(test)]
#[path = "cockpit.tests.rs"]
mod tests;
