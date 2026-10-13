//! Work started inside a live hosted session.
//!
//! A host alternates two phases on one command channel: the conversation
//! ([`InteractiveSession`]) and, when the reader starts one item or the loop,
//! the same [`IterationLoop`] and [`LoopDriver`] the terminal's `next` and
//! `next-loop` run. The host's admitted writer is the loop's writer; a host
//! that only attached to another owner, or a project whose loop is already
//! owned, refuses the start in words. The run is prepared while the
//! conversation is still open — that is the session's admission question —
//! so a refused start costs the reader nothing.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use magistr_cockpit::cockpit::{Cmd, LaunchAction, Msg};
use magistr_cockpit_runtime::InteractiveSession;
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_core::workflow::loop_lease::LoopLease;
use magistr_engine::NativeEngines;
use magistr_engine::workflow::RunMode;
use tokio::sync::mpsc::UnboundedReceiver;

use super::goal::{GoalMode, Judge, session_judge_for};
use super::interactive_session::interactive_session;
use super::ledger::{SpinePort, session_engine};
use super::loop_driver::{Connector, LoopDriver};
use crate::agent::{AgentConfig, Backend};
use crate::application::workflow_client::EngineClient;
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::IterationLoop;

/// Iterations a started loop may run — the `next-loop` default.
const LOOP_ITERATIONS: u32 = 10;

/// Who judges a started run: the goal mode, and whether the project is native.
pub(crate) type JudgeSource = dyn Fn(
        GoalMode,
        bool,
        &Path,
        Backend,
        &crate::config::inputs::ProjectInputs,
    ) -> Option<Arc<dyn Judge>>
    + Send;

/// The phases of one hosted session.
pub(crate) struct HostedWork {
    config: AgentConfig,
    connector: Connector,
    inbox: Inbox,
    engines: EngineClient,
    /// The engine feed, tapped beside the inbox: a running driver counts the
    /// chunks its iteration completes from it; between runs it is drained.
    feed: UnboundedReceiver<Msg>,
    backend: Backend,
    judge: Box<JudgeSource>,
    /// The run the open conversation admitted, until the work phase takes it.
    admitted: Arc<Mutex<Option<Prepared>>>,
}

/// A run ready for its driver; the lease is held until the run is dropped.
struct Prepared {
    wf: WorkflowConfig,
    iter_loop: IterationLoop,
    persist: bool,
    _lease: Option<LoopLease>,
}

impl HostedWork {
    pub(crate) fn new(
        config: AgentConfig,
        connector: Connector,
        inbox: Inbox,
        engines: EngineClient,
        feed: UnboundedReceiver<Msg>,
    ) -> Self {
        Self {
            backend: config.backend,
            config,
            connector,
            inbox,
            engines,
            feed,
            judge: Box::new(session_judge_for),
            admitted: Arc::default(),
        }
    }

    /// Decide who judges a started run (tests script the verdicts).
    #[cfg(test)]
    pub(crate) fn judged_by(mut self, judge: Box<JudgeSource>) -> Self {
        self.judge = judge;
        self
    }

    /// Serve `commands` until the cockpit closes: `opening` first, then work
    /// and a fresh conversation in turn, for as long as the reader starts work.
    pub(crate) async fn run(
        mut self,
        opening: InteractiveSession,
        mut commands: Commands,
    ) -> Result<(), String> {
        let mut session = self.admitting(opening);
        loop {
            let exit = {
                let phase = session.run(&mut commands, None, false);
                tokio::pin!(phase);
                loop {
                    tokio::select! {
                        exit = &mut phase => break exit,
                        Some(_) = self.feed.recv() => {}
                    }
                }
            };
            let (true, Some(action)) = (exit.open, exit.launch) else {
                return Ok(());
            };
            commands = match self.work(action, commands).await {
                Some(commands) => commands,
                None => return Ok(()),
            };
            session = self.admitting(interactive_session(
                self.conversation(),
                self.connector.clone(),
                self.inbox.clone(),
                Some(self.engines.clone()),
            ));
        }
    }

    /// `session`, asking this host before it gives the conversation up to a
    /// start: the run is opened here, and kept for the work phase.
    fn admitting(&self, session: InteractiveSession) -> InteractiveSession {
        let dir = self.config.cwd.clone();
        let engines = self.engines.clone();
        let admitted = Arc::clone(&self.admitted);
        session.admitting_work(move |action| {
            let prepared = prepare(&dir, &engines, action)?;
            *admitted.lock().unwrap_or_else(PoisonError::into_inner) = Some(prepared);
            Ok(())
        })
    }

    /// The configuration of a conversation opened after work: the opening
    /// prompt and any resumed identity belonged to the first one.
    fn conversation(&self) -> AgentConfig {
        let mut config = self.config.clone();
        config.prompt.clear();
        config.history = crate::agent::persistence::SessionHistory::new();
        config.resume = None;
        config.continue_conversation = false;
        config
    }

    /// Run the admitted work to its end on the shared driver and return the
    /// commands, or `None` once the cockpit is gone.
    async fn work(&mut self, action: LaunchAction, commands: Commands) -> Option<Commands> {
        let admitted = self
            .admitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(prepared) = admitted else {
            self.inbox.send(Msg::CommandFailed {
                cmd: Cmd::Launch(action),
                error: "The work was not prepared. Start it again.".into(),
            });
            return Some(commands);
        };
        let dir = self.config.cwd.clone();
        let inputs = self.config.inputs().for_project(&dir);
        let auto_pause = inputs
            .user_config()
            .map(|cfg| cfg.value.general.auto_pause_or_default())
            .unwrap_or(true);
        let (goal, goal_note) = GoalMode::from_value(inputs.goal_mode());
        let native = prepared.wf.is_native();
        let mut driver = LoopDriver::new(
            prepared.iter_loop,
            prepared.wf,
            self.backend,
            self.inbox.clone(),
            commands,
            self.connector.clone(),
        )
        .with_inputs(&inputs)
        .persisting(prepared.persist)
        .auto_pause(auto_pause)
        .goal(
            goal,
            (self.judge)(goal, native, &dir, self.backend, &inputs),
            goal_note,
        )
        .checkpoints(Box::new(SpinePort::at(
            session_engine(&dir, self.engines.reader().clone()),
            &dir,
        )));
        if let Some(profile) = self.config.permission_profile {
            driver = driver.permission_profile(profile);
        }
        let tap = driver.tap();
        let run = driver.run_to_end();
        tokio::pin!(run);
        let (report, commands) = loop {
            tokio::select! {
                done = &mut run => break done,
                Some(message) = self.feed.recv() => {
                    // The driver listens for as long as it runs.
                    let _ = tap.send(message);
                }
            }
        };
        tracing::info!(%report, "hosted work ended");
        commands
    }
}

/// Open the run the terminal entry points open, on this host's writer.
fn prepare(dir: &Path, engines: &EngineClient, action: &LaunchAction) -> Result<Prepared, String> {
    let writer = engines.writer().cloned().ok_or_else(|| {
        "Another Magistr session owns this project's work. Start the work there, \
             or close it and reopen this one."
            .to_owned()
    })?;
    let native = magistr_core::roadmap::backend_for(dir)
        .map_err(|error| error.to_string())?
        .is_native()
        .then_some(writer);
    let wf = WorkflowConfig::load(dir)
        .map_err(|error| error.to_string())?
        .with_native(native.as_ref().map(NativeEngines::reader));
    match action {
        LaunchAction::Next => {
            if wf
                .spec_summary()
                .map_err(|error| error.to_string())?
                .all_done()
            {
                return Err("Every roadmap item is complete. There is nothing to start.".into());
            }
            wf.cleanup().map_err(|error| error.to_string())?;
            let iter_loop = IterationLoop::open(dir, &wf.spec_path, 1, native)
                .map_err(|error| error.to_string())?
                .with_run_mode(RunMode::OneChunk);
            Ok(Prepared {
                wf,
                iter_loop,
                persist: false,
                _lease: None,
            })
        }
        LaunchAction::Loop(scope) => {
            let lease = LoopLease::acquire(dir).map_err(|error| error.to_string())?;
            let mut iter_loop =
                IterationLoop::start_or_recover(dir, &wf.spec_path, LOOP_ITERATIONS, native)
                    .map_err(|error| error.to_string())?;
            iter_loop
                .select_track(scope.track())
                .map_err(|error| error.to_string())?;
            iter_loop.persist().map_err(|error| error.to_string())?;
            Ok(Prepared {
                wf,
                iter_loop,
                persist: true,
                _lease: Some(lease),
            })
        }
        _ => Err("This session cannot start that here.".into()),
    }
}

#[cfg(test)]
#[path = "hosted_work.tests.rs"]
mod tests;
