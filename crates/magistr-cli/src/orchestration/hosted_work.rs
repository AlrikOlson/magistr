//! Work started inside a live hosted session.
//!
//! A host alternates two phases on one command channel: the conversation
//! ([`InteractiveSession`]) and, when the reader starts one item or the loop,
//! progress tracker as the terminal's `next-loop`, with [`LoopDriver`] owning
//! hosted checks and judging. The driver uses the host's admitted writer; a host
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
    observation: Option<Arc<crate::application::session_observation::SessionObservation>>,
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
    native: Option<NativeEngines>,
    persist: bool,
    lease: Arc<LoopLease>,
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
            observation: None,
            feed,
            judge: Box::new(session_judge_for),
            admitted: Arc::default(),
        }
    }

    pub(crate) fn observing(
        mut self,
        observation: Option<Arc<crate::application::session_observation::SessionObservation>>,
    ) -> Self {
        self.observation = observation;
        self
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
        let mut session = self.admitting(opening, self.config.history.clone());
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
            self.config = self.conversation();
            let config = self.config.clone();
            let (transport, spec) = self
                .connector
                .connect(config.clone())
                .await
                .map_err(|error| error.to_string())?;
            let handle = self
                .connector
                .start_session(
                    transport,
                    spec,
                    magistr_application::sessions::SessionOptions::new(),
                )
                .await
                .map_err(|error| error.to_string())?;
            session = self.admitting(
                interactive_session(
                    config,
                    self.connector.clone(),
                    self.inbox.clone(),
                    Some(self.engines.clone()),
                )
                .connected(Box::new(handle), false),
                self.config.history.clone(),
            );
        }
    }

    /// `session`, asking this host before it gives the conversation up to a
    /// start: the run is opened here, and kept for the work phase.
    fn admitting(
        &self,
        session: InteractiveSession,
        history: crate::agent::persistence::SessionHistory,
    ) -> InteractiveSession {
        let dir = self.config.cwd.clone();
        let engines = self.engines.clone();
        let admitted = Arc::clone(&self.admitted);
        let selection = self.config.inputs().session_selection().cloned();
        let observation = self.observation.clone();
        session.admitting_work(move |action| {
            let mut selection = selection.clone();
            if let Some(selection) = &mut selection {
                // Read the configuration the user is looking at when work is
                // admitted, including model changes and parked conversations.
                selection.model = observation
                    .as_ref()
                    .map_or_else(
                        || history.current_model(),
                        |observation| observation.snapshot().model,
                    )
                    .filter(|model| !model.eq_ignore_ascii_case("default"));
                selection
                    .sources
                    .insert("model".into(), "current conversation".into());
            }
            let prepared = prepare(&dir, &engines, action, selection.as_ref())?;
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
        config.model = None;
        config.effort = None;
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
        let mut inputs = self.config.inputs().for_project(&dir);
        if let Some(selection) = &prepared.iter_loop.state().session_selection {
            inputs = inputs.with_session_selection(selection.clone());
        }
        self.config = self.config.clone().with_inputs(&inputs);
        let auto_pause = inputs
            .user_config()
            .map(|cfg| cfg.value.general.auto_pause_or_default())
            .unwrap_or(true);
        let (goal, goal_note) = GoalMode::from_value(inputs.goal_mode());
        let native = prepared.wf.is_native();
        let mut driver = LoopDriver::new(
            prepared.iter_loop,
            prepared.native,
            prepared.wf,
            self.backend,
            self.inbox.clone(),
            commands,
            self.connector.clone(),
        )
        .holding_lease(Arc::clone(&prepared.lease))
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
fn prepare(
    dir: &Path,
    engines: &EngineClient,
    action: &LaunchAction,
    selection: Option<&crate::config::session_selection::SessionSelection>,
) -> Result<Prepared, String> {
    let writer = engines.writer().cloned().ok_or_else(|| {
        "Another Magistr session owns this project's work. Start the work there, \
             or close it and reopen this one."
            .to_owned()
    })?;
    let lease = Arc::new(LoopLease::acquire(dir).map_err(|error| error.to_string())?);
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
            let mut iter_loop = IterationLoop::start_or_recover(
                dir,
                &wf.spec_path,
                1,
                native.as_ref().map(NativeEngines::reader),
            )
            .map_err(|error| error.to_string())?;
            if iter_loop.is_recovered() && iter_loop.run_state().mode != RunMode::OneChunk {
                return Err(
                    "A longer work run is unfinished. Continue that run before starting one item."
                        .into(),
                );
            }
            iter_loop = iter_loop.with_run_mode(RunMode::OneChunk);
            if let Some(selection) = selection {
                iter_loop
                    .bind_session_selection(selection.clone())
                    .map_err(|error| error.to_string())?;
            }
            iter_loop.persist().map_err(|error| error.to_string())?;
            Ok(Prepared {
                wf,
                iter_loop,
                native,
                persist: true,
                lease,
            })
        }
        LaunchAction::Loop(scope) => {
            let mut iter_loop = IterationLoop::start_or_recover(
                dir,
                &wf.spec_path,
                LOOP_ITERATIONS,
                native.as_ref().map(NativeEngines::reader),
            )
            .map_err(|error| error.to_string())?;
            if iter_loop.is_recovered() && iter_loop.run_state().mode != RunMode::ChunkLoop {
                return Err("A single work item is unfinished. Continue that item before starting a longer run.".into());
            }
            iter_loop
                .select_track(scope.track())
                .map_err(|error| error.to_string())?;
            if let Some(selection) = selection {
                iter_loop
                    .bind_session_selection(selection.clone())
                    .map_err(|error| error.to_string())?;
            }
            iter_loop.persist().map_err(|error| error.to_string())?;
            Ok(Prepared {
                wf,
                iter_loop,
                native,
                persist: true,
                lease,
            })
        }
        _ => Err("This session cannot start that here.".into()),
    }
}

#[cfg(test)]
#[path = "hosted_work.tests.rs"]
mod tests;
