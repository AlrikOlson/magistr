//! `magistr next` and `next-loop` on the mission-control cockpit
//! (`crates/magistr-tui`): the loop driver beside the runtime, the
//! iteration's turn on the stage, the loop on the status strip.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::application::workflow_client::EngineClient;
use color_eyre::eyre::{self, Context as _};
use magistr_application::effects::{SourceRefusal, SourceRoute};
use magistr_cockpit::cockpit::{Model, Summary};
use magistr_cockpit::theme::Theme;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RealTerminal, Terminal as _};

use crate::agent::Backend;
use crate::orchestration::goal::{GoalMode, Judge, SessionJudge};
use crate::orchestration::ledger::{SpinePort, session_engine};
use crate::orchestration::loop_driver::{Connector, LoopDriver};
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::IterationLoop;

/// How long the driver gets to wind down once the cockpit is gone.
const LOOP_GRACE: Duration = Duration::from_secs(5);

/// Run `iter_loop` on the cockpit until it ends or the user quits.
///
/// What can fail loudly happened on the normal terminal already: on a
/// native project the entry point admitted this process as the host and
/// opened the loop's engines, and the cockpit attaches to that one writer
/// rather than opening a second cached one. The host's owner endpoint is
/// served here for the run, so a `magistr run` in another terminal acts
/// through this writer. Then the terminal is taken over, the driver spawned beside
/// the runtime (the feed tapped so the driver sees the chunks an
/// iteration completes), and the cockpit run; leaving restores the
/// terminal, the driver ends with its session, and the summary and the
/// loop's report land in the scrollback. `persist` keeps the loop's
/// state on disk between iterations (`next-loop`); `magistr next` runs
/// its one iteration without it.
pub(crate) async fn run_cockpit_loop(
    dir: &Path,
    wf: WorkflowConfig,
    iter_loop: IterationLoop,
    backend: Backend,
    persist: bool,
    profile: crate::permissions::PermissionProfile,
    inputs: &crate::config::inputs::ProjectInputs,
) -> eyre::Result<()> {
    // Served before raw mode so its refusal is readable; dropped after the
    // runtime, before the loop's writer.
    let owner_endpoint = crate::roadmap::serve_owner_endpoint(iter_loop.native()).await?;
    let engines = iter_loop.native().cloned().map(EngineClient::attach);
    // The workspace this host owns for the loop: the sessions' authorized
    // writes run through its pipeline (or are refused when it could not be
    // admitted); the agents' bridges attach to it.
    let workspace =
        crate::orchestration::commands::run::admit_workspace(inputs, |line| println!("{line}"))
            .await;
    let source = workspace.source_route();
    let auto_pause = inputs
        .user_config()
        .map(|cfg| cfg.value.general.auto_pause_or_default())
        .unwrap_or(true);
    let (goal, goal_note) = GoalMode::from_env();
    let native = wf.is_native();

    let theme = Theme::from_os();
    let (title, debug) =
        crate::orchestration::commands::chrome::chrome(dir, Some(backend.label()), None);
    let terminal = RealTerminal::stdio().context("open Magistr in a terminal to use this view")?;
    let size = terminal.size()?;
    let mut runtime = Runtime::new(terminal, Options::from_os());
    let commands = runtime
        .commands()
        .ok_or_else(|| eyre::eyre!("Magistr could not connect the session controls"))?;
    let driver = LoopDriver::new(
        iter_loop,
        wf,
        backend,
        runtime.inbox(),
        commands,
        Connector::live(source),
    )
    .with_inputs(inputs)
    .permission_profile(profile)
    .keep_open(true)
    .persisting(persist)
    .auto_pause(auto_pause)
    .goal(goal, judge(goal, native, dir, backend, inputs), goal_note);
    // The ledger the loop checkpoints on at every iteration boundary —
    // served by the driver off its own thread, never by a session.
    let driver = match &engines {
        Some(engines) => driver.checkpoints(Box::new(SpinePort::at(
            session_engine(dir, engines.reader().clone()),
            dir,
        ))),
        None => driver,
    };
    let seed = engines.as_ref().map(|engines| engines.reader().snapshot());
    // The socket carries the driver's own iterations; the timer is the
    // safety net for a store written while nothing is attached (the
    // driver re-seeds only at each iteration start), measured against
    // the snapshot the model is seeded with.
    let (feed, reseed) = match (&engines, &seed) {
        (Some(engines), Some(seed)) => (
            Some(engines.subscribe(runtime.inbox().tap(driver.tap()))),
            Some(engines.reseed(runtime.inbox(), seed.cursor)),
        ),
        _ => (None, None),
    };
    let looping = driver.spawn();
    let mut model = Model::new(size)
        .with_tab(magistr_cockpit::cockpit::StageTab::Turn)
        .with_theme(theme)
        .with_title(title)
        .with_debug(debug)
        .with_workspace_files(
            crate::orchestration::commands::workspace_files::workspace_files(dir),
        );
    if let Some(seed) = &seed {
        model = model.with_snapshot(seed);
    }
    // The rail is governed the way the driver's sessions are: the
    // project's profile under workflow mode — the terminal draws none
    // of it, but the one seeder binds every entry point alike.
    let model =
        crate::orchestration::commands::run::seed_supervision(model, dir, profile, true, inputs);
    let model = runtime.run(model).await;
    if let Some(feed) = feed {
        feed.abort();
    }
    if let Some(reseed) = reseed {
        reseed.abort();
    }
    drop(owner_endpoint);
    // The runtime is gone, so the driver's commands have ended: it stops
    // at its next wake and drops its session, which ends the agent. Give
    // it a moment, never wait forever on a stuck child.
    let report = tokio::time::timeout(LOOP_GRACE, looping).await;
    if let Err(err) = workspace.shutdown().await {
        println!("magistr: the project connection could not close cleanly: {err}");
    }
    let model = model.context("the session ended unexpectedly")?;
    println!("{}", Summary::of(&model));
    if let Ok(Ok(report)) = report {
        println!("{report}");
    }
    Ok(())
}

/// Who judges an iteration: magistr's own fresh session ([`SessionJudge`])
/// on a roadmap-driven project under the native mode — and under the
/// delegated modes too, as what an unconfirmed delivery falls back to;
/// nobody under `off`, and nobody on a project with no chunk to judge.
/// The judge is constrained: it holds no source-write authority, so its
/// session advertises no write capability and refuses any write.
fn judge(
    goal: GoalMode,
    native: bool,
    dir: &Path,
    backend: Backend,
    inputs: &crate::config::inputs::ProjectInputs,
) -> Option<Arc<dyn Judge>> {
    match goal {
        GoalMode::Off => None,
        GoalMode::Native | GoalMode::Extension | GoalMode::Command | GoalMode::Auto if native => {
            Some(Arc::new(
                SessionJudge::new(
                    Connector::live(SourceRoute::Refused(SourceRefusal::NoAuthority)),
                    dir,
                    backend,
                )
                .with_inputs(inputs),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "run_cockpit_loop.tests.rs"]
mod tests;
