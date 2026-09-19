//! The task that walks the loop.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::checkpoint_detach::detach;
use crate::agent::cockpit_session::CockpitSession;
use crate::agent::session_supervisor::SessionControl;
use crate::application::workflow_client::NativeEnginePort;
use magistr_acp::AcpEvent;
use magistr_application::sessions::{AgentEvent, SessionOptions, TurnEnd};
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_cockpit::cockpit::{CheckpointCmd, Cmd, KillCmd, LoopCmd, LoopEvent, LoopStage, Msg};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit_runtime::Jobs;
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_cockpit_runtime::spawn_session;
use magistr_engine::native::{BlockerKind, CheckSettlement, Chunk};
use magistr_engine::{ChunkStatus, NativeEngines, NativeEvent, RoadmapFrame};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use super::connector::Connector;
use super::failure_brief::failure_brief;
use super::gate_job::gate_job;
use super::gate_outcome::GateOutcome;
use super::iteration_metrics::IterationMetrics;
use super::loop_end::LoopEnd;
use super::loop_report::LoopReport;
use crate::agent::{AgentConfig, Backend, ResolvedSessionSpec, ToolPermissions};
use crate::application::AdmittedCheck;
use crate::orchestration::goal::{
    AgentGoal, Decision, Delegation, Delivery, DeliveryKind, GateEvidence, GoalMode, Judge,
    JudgeInput,
};
use crate::permissions::PermissionProfile;
use crate::utils::gate::GateVerdict;
use crate::workflow::WorkflowConfig;
use crate::workflow::iteration::{
    AttemptId, IterationLoop, OperationKind, OperationOutcome, PostCheckResult, PreCheckResult,
};
use crate::workflow::runtime_handle::RuntimeHandle;
use magistr_core::evidence::WorkspaceEvidence;

/// How long a session gets to end its agent once its turn is over.
const SESSION_GRACE: Duration = Duration::from_secs(5);
/// How often the strip's clock is refreshed while an iteration runs.
const TICK: Duration = Duration::from_secs(1);

/// How long a delegated session gets to complete its handshake before
/// the attempt is called off.
const NEGOTIATE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long after the handshake the command list may still arrive — the
/// agent sends `available_commands_update` on its own timer right after
/// `session/new`.
const COMMANDS_GRACE: Duration = Duration::from_secs(1);

/// How long a delivered goal gets to be confirmed by the optimistic
/// echo. The echo arrives ~3 ms after a recognised delivery; a wait
/// this long is already a no.
const ECHO_TIMEOUT: Duration = Duration::from_secs(5);

/// A moment for an abandoning `clear` to reach the wire before its
/// session is dropped.
const CLEAR_GRACE: Duration = Duration::from_millis(300);

/// How long the cockpit gets to ask for the boundary checkpoint it was
/// told to mint — one fold and one command back, normally milliseconds.
const LEDGER_ASK_GRACE: Duration = Duration::from_secs(2);

/// How long a boundary checkpoint gets to land before the loop goes on
/// without waiting — one `git status` on a large tree, not a hung one.
const LEDGER_SAVE_GRACE: Duration = Duration::from_secs(10);

/// The iteration loop as a task beside the cockpit's runtime.
///
/// Built from the loop's state and configuration, the runtime's
/// [`Inbox`] and [`Commands`], and a [`Connector`]; [`LoopDriver::spawn`]
/// runs it until the loop ends or the cockpit closes. Per iteration it
/// checks the loop's preconditions, honours a pause / skip / abort the
/// user asked for at the boundary, builds the prompt, connects the agent
/// and runs the session until the turn ends, then runs the gates as
/// jobs, records the commit check and — by configuration — pauses on
/// failed gates, and advances (persisting the loop's state when asked).
///
/// With a [`Judge`] ([`LoopDriver::goal`]) it is the native goal loop:
/// after the gates it asks the judge about the iteration's
/// chunk and acts on the reconciled [`Decision`] — completes the chunk
/// through the engines, iterates with the reason on the stage, reports
/// a disagreement, or blocks the chunk on a human and ends.
pub struct LoopDriver {
    iter_loop: IterationLoop,
    wf: WorkflowConfig,
    backend: Backend,
    permission_profile: Option<PermissionProfile>,
    inbox: Inbox,
    commands: Commands,
    jobs: Jobs,
    connector: Connector,
    persist: bool,
    auto_pause: bool,
    keep_open: bool,
    session_active: bool,
    roadmap_request: Option<String>,
    goal: GoalMode,
    goal_note: Option<String>,
    judge: Option<Arc<dyn Judge>>,
    /// What delegation settled to, once negotiated (delegated modes only).
    delegated: Option<DelegationOutcome>,
    /// How long a delivered goal may wait for its echo.
    echo_timeout: Duration,
    tap_tx: UnboundedSender<Msg>,
    tap_rx: UnboundedReceiver<Msg>,
    paused: bool,
    /// A step is running: the boundary after this iteration pauses.
    stepping: bool,
    pending: Option<LoopCmd>,
    completed: Vec<String>,
    /// The chunk the iteration's prompt was built for.
    target: Option<String>,
    /// When this iteration began, on the wall clock the store stamps its
    /// records with: an objective set after it is this iteration's work.
    began: Option<chrono::DateTime<chrono::Utc>>,
    /// The part the agent shipped instead, when it split the target and its
    /// recorded checks verified that part: the target itself is then still
    /// to do, and is not judged on the part's evidence.
    split_part: Option<String>,
    /// The execution task this iteration's gates were recorded on — the
    /// one active in the trace as the set began — so a completion the
    /// judge grants names the task its evidence sits on. `None` when the
    /// trace had no task to record on (the gates then went unrecorded).
    verification_task: Option<String>,
    /// Passing recorded checks revalidated against current source this iteration.
    recorded_verification: Vec<String>,
    /// Why this iteration's gate set could not be resolved, when it could
    /// not: the iteration then failed, whatever ran. One-shot, read by the
    /// boundary that settles the set.
    gates_unresolved: Option<String>,
    /// What the last iteration's red gates left for the next prompt: the
    /// failure rows themselves, not a sentence about them. One-shot —
    /// appended to the next prompt and cleared.
    carry: Option<String>,
    /// The agent's own words this iteration — the judge's transcript tail.
    transcript: String,
    iterations: u32,
    /// What each iteration cost, for the report.
    metrics: Vec<IterationMetrics>,
    /// The highest context level the agent reported this turn.
    tokens_seen: Option<u64>,
    /// Spend accumulated before this attempt's fresh provider session.
    iteration_cost_base: f64,
    /// The session's checkpoint spine, when the loop runs on a native
    /// project: what the boundary checkpoint is taken on, and what
    /// serves every other ledger verb the loop's cockpit asks for.
    /// Shared with the blocking thread that runs a verb, as the
    /// session's is.
    checkpoints: Option<Arc<Mutex<Box<dyn CheckpointPort>>>>,
    /// The ledger verb in flight, if one is — what a boundary waits for.
    ledger: Option<JoinHandle<()>>,
}

/// One gate's admission and the operation record that will settle on it.
#[derive(Clone)]
struct Recording {
    admission: AdmittedCheck,
    receipt: Option<AttemptId>,
}

/// What writing an operation's intent came to.
enum Intent {
    /// A new attempt, durable before its effect.
    Attempt(AttemptId),
    /// The command's outcome is already established this attempt: the
    /// effect is not redone.
    Established,
    /// The intent could not be written; the note says why.
    Unrecorded(String),
}

/// What the driver woke up for.
// `Tap` carries a cockpit `Msg`, which carries a whole store record (see
// `NativeEvent`); a wake is matched and consumed at once, never kept, so
// boxing it would buy an allocation per message and no memory.
#[allow(clippy::large_enum_variant)]
enum Wake {
    /// A message a tapped producer posted on the cockpit.
    Tap(Msg),
    /// A command from the cockpit.
    Cmd(Cmd),
    /// A ledger verb was taken up on the driver's own port, off this
    /// task; its answer reaches the cockpit when it lands.
    Ledger,
    /// A gate job returned.
    Gate(GateOutcome),
    /// Every gate job is gone without returning.
    GatesGone,
    /// The clock ticked.
    Tick,
    /// The cockpit is gone.
    Closed,
}

/// What the boundary before an iteration decided.
enum Boundary {
    Go,
    Skip,
    Abort,
    Closed,
}

/// How an agent turn ended.
enum Turn {
    Ended,
    Failed(String),
    Closed,
}

/// What an iteration led to.
enum Step {
    Next,
    Closed,
    End(LoopEnd),
}

/// What the loop has settled about delegation — negotiated once, then
/// remembered for every later iteration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DelegationOutcome {
    /// A delivery was confirmed by the echo; later iterations use the
    /// same kind directly.
    Chosen(DeliveryKind),
    /// No delivery confirmed; the native loop drives from here on.
    NativeFallback,
}

/// What a delegated turn came to.
enum Delegated {
    /// The agent's own goal loop ran the iteration to its end.
    Ran,
    /// No delivery was confirmed — the caller runs the native turn.
    Fallback,
    /// The cockpit is gone.
    Closed,
}

/// One delivery attempt's outcome.
enum Attempt {
    /// The echo confirmed the delivery, and the agent's loop ran to its
    /// end.
    Confirmed(DeliveryKind),
    /// This session does not offer the vehicle at all.
    Unavailable(String),
    /// Issued, never echoed; cleared and abandoned.
    Unconfirmed(String),
    /// The cockpit is gone.
    Closed,
}

/// The relay task, the cockpit command sender and the owner's goal control.
type OpenedSession = (
    JoinHandle<Result<(), String>>,
    UnboundedSender<Cmd>,
    SessionControl,
);

impl LoopDriver {
    /// A driver over `iter_loop` for the project `wf` describes, running
    /// agents on `backend` through `connector`, reporting on `inbox` and
    /// deciding by `commands`. Persistence and auto-pause are on.
    #[must_use]
    pub fn new(
        iter_loop: IterationLoop,
        wf: WorkflowConfig,
        backend: Backend,
        inbox: Inbox,
        commands: Commands,
        connector: Connector,
    ) -> Self {
        let (tap_tx, tap_rx) = unbounded_channel();
        Self {
            jobs: Jobs::new(inbox.clone()),
            iter_loop,
            wf,
            backend,
            permission_profile: None,
            inbox,
            commands,
            connector,
            persist: true,
            auto_pause: true,
            keep_open: false,
            session_active: false,
            roadmap_request: None,
            goal: GoalMode::Off,
            goal_note: None,
            judge: None,
            delegated: None,
            echo_timeout: ECHO_TIMEOUT,
            carry: None,
            tap_tx,
            tap_rx,
            paused: false,
            stepping: false,
            pending: None,
            completed: Vec::new(),
            target: None,
            began: None,
            split_part: None,
            verification_task: None,
            recorded_verification: Vec::new(),
            gates_unresolved: None,
            transcript: String::new(),
            iterations: 0,
            metrics: Vec::new(),
            tokens_seen: None,
            iteration_cost_base: 0.0,
            checkpoints: None,
            ledger: None,
        }
    }

    /// Use the profile explicitly selected by the launching interface.
    #[must_use]
    pub fn permission_profile(mut self, profile: PermissionProfile) -> Self {
        self.permission_profile = Some(profile);
        self
    }

    /// The ledger the loop checkpoints on: one state per iteration
    /// boundary, wearing that iteration's gate verdicts, plus whatever
    /// ledger verb the loop's cockpit asks for meanwhile. Without one
    /// the loop takes no checkpoints and the spine stays empty.
    #[must_use]
    pub fn checkpoints(mut self, port: Box<dyn CheckpointPort>) -> Self {
        self.checkpoints = Some(Arc::new(Mutex::new(port)));
        self
    }

    /// Who drives the goal loop: `mode` is what the cockpit is told, and
    /// `judge`, when there is one, is asked after every iteration's gates
    /// on a native project. A delegated mode hands each iteration to the
    /// agent's own `/goal` (agent_goal.rs), confirmed per delivery by the
    /// optimistic echo; when no delivery confirms, the loop reports it
    /// and falls back to this same judge. Without a judge the driver
    /// runs the plain loop (`MAGISTR_GOAL=off`). `note` is what parsing
    /// the mode had to say (an unknown value), put on the stage ahead of
    /// the first iteration.
    #[must_use]
    pub fn goal(
        mut self,
        mode: GoalMode,
        judge: Option<Arc<dyn Judge>>,
        note: Option<String>,
    ) -> Self {
        self.goal = mode;
        self.judge = judge;
        self.goal_note = note;
        self
    }

    /// The line that says which mode drives, and whether a judge sits
    /// behind it.
    fn goal_line(&self) -> String {
        let mode = self.goal.label();
        match (self.goal, self.judge.is_some()) {
            (GoalMode::Off, _) => "goal loop: off — one iteration at a time, no judge".to_owned(),
            (GoalMode::Native, true) => {
                format!("goal loop: {mode} — magistr's own judge decides after the gates")
            }
            (GoalMode::Native, false) => format!(
                "goal loop: {mode} — no judge this loop (the project has no roadmap chunk to judge)"
            ),
            (_, fallback) => format!(
                "goal loop: {mode} — delegating to the agent's own /goal, confirmed by its echo; magistr is blind inside it, and no echo falls back to {}",
                if fallback {
                    "the native judge"
                } else {
                    "plain iterations"
                }
            ),
        }
    }

    /// Wait at most this long for the optimistic echo after a delivery
    /// (tests shrink it; the default is [`ECHO_TIMEOUT`]).
    #[must_use]
    pub const fn goal_echo_timeout(mut self, timeout: Duration) -> Self {
        self.echo_timeout = timeout;
        self
    }

    /// Whether this iteration should try (or keep using) a delegated
    /// delivery.
    const fn delegating(&self) -> bool {
        self.goal.is_delegated()
            && !matches!(self.delegated, Some(DelegationOutcome::NativeFallback))
    }

    /// The loop's record of who drove the goal loop — `None` unless a
    /// delegated mode was asked for; the negotiated outcome otherwise.
    fn goal_record(&self) -> Option<String> {
        if !self.goal.is_delegated() {
            return None;
        }
        Some(match self.delegated {
            Some(DelegationOutcome::Chosen(kind)) => format!("delegated ({})", kind.label()),
            Some(DelegationOutcome::NativeFallback) => {
                format!("{} → native (no echo)", self.goal.label())
            }
            None => format!("{} (never negotiated)", self.goal.label()),
        })
    }

    /// Whether the loop's state is written to `.magistr/loop-state.json`
    /// after every iteration and removed at the end (`magistr next`, the
    /// single iteration, keeps no state).
    #[must_use]
    pub const fn persisting(mut self, persist: bool) -> Self {
        self.persist = persist;
        self
    }

    /// Whether failed gates pause the loop at the next boundary.
    #[must_use]
    pub const fn auto_pause(mut self, auto_pause: bool) -> Self {
        self.auto_pause = auto_pause;
        self
    }

    /// Keep accepting conversation and roadmap requests after the work loop ends.
    #[must_use]
    pub const fn keep_open(mut self, keep_open: bool) -> Self {
        self.keep_open = keep_open;
        self
    }

    /// The observer for a tapped inbox: what a producer posts through
    /// `inbox.tap(driver.tap())` reaches the driver too — the entry
    /// point subscribes the engine feed this way so the driver counts the
    /// chunks an iteration completes.
    #[must_use]
    pub fn tap(&self) -> UnboundedSender<Msg> {
        self.tap_tx.clone()
    }

    /// Run the loop as a task; the report comes back when it ends.
    pub fn spawn(mut self) -> JoinHandle<LoopReport> {
        tokio::spawn(async move { self.run().await })
    }

    /// Run the loop in place and hand the cockpit's commands back with the
    /// report: a host that starts work inside a live session goes on
    /// serving that session when the work ends. The second value is `None`
    /// when the cockpit closed meanwhile.
    pub(crate) async fn run_to_end(mut self) -> (LoopReport, Option<Commands>) {
        let report = self.run().await;
        let open = *report.end() != LoopEnd::CockpitClosed
            && !crate::utils::shutdown::Shutdown::is_requested();
        (report, open.then_some(self.commands))
    }

    async fn run(&mut self) -> LoopReport {
        let end = loop {
            // An interrupt admits no further attempt: the one in flight
            // has already been wound down by its session owner.
            if crate::utils::shutdown::Shutdown::is_requested() {
                break LoopEnd::Aborted;
            }
            let pre = match self.iter_loop.pre_iteration_check() {
                Ok(pre) => pre,
                Err(e) => break LoopEnd::Failed(format!("pre-iteration check failed: {e}")),
            };
            match pre {
                PreCheckResult::Continue => {}
                PreCheckResult::Stopped(reason) => {
                    break LoopEnd::from_stop(reason, self.iter_loop.max_iterations());
                }
                PreCheckResult::NoReadyChunk => break LoopEnd::NoReadyChunk,
                PreCheckResult::SpecComplete => break LoopEnd::SpecComplete,
                PreCheckResult::MaxIterations => {
                    break LoopEnd::MaxIterations(self.iter_loop.max_iterations());
                }
                PreCheckResult::Unreconciled(open) => {
                    let open: Vec<String> = open
                        .iter()
                        .map(|record| {
                            format!(
                                "{} {} (attempt {}{})",
                                record.kind,
                                record.target,
                                record.attempt.number(),
                                record
                                    .external
                                    .as_deref()
                                    .map(|handle| format!(", {handle}"))
                                    .unwrap_or_default()
                            )
                        })
                        .collect();
                    break LoopEnd::Unreconciled(open.join("; "));
                }
            }
            if self.persist
                && let Err(error) = self.iter_loop.persist()
            {
                break LoopEnd::Failed(format!("could not checkpoint attempt: {error}"));
            }
            match self.boundary().await {
                Boundary::Go => {}
                Boundary::Skip => {
                    self.note(format!(
                        "skipped iteration {}",
                        self.iter_loop.display_iteration()
                    ));
                    self.advance();
                    continue;
                }
                Boundary::Abort => break LoopEnd::Aborted,
                Boundary::Closed => break LoopEnd::CockpitClosed,
            }
            match self.iteration().await {
                Ok(Step::Next) => {}
                Ok(Step::Closed) => break LoopEnd::CockpitClosed,
                Ok(Step::End(end)) => break end,
                Err(why) => {
                    self.iter_loop.post_iteration_check();
                    self.iter_loop.mark_failed();
                    break LoopEnd::Failed(why);
                }
            }
            if let Some(text) = self.roadmap_request.take()
                && !self.interact(Some(Cmd::RoadmapPrompt(text)), true).await
            {
                break LoopEnd::CockpitClosed;
            }
            self.advance();
        };
        if self.iter_loop.run_state().last_settled_attempt
            == Some(self.iter_loop.run_state().completed_attempts)
        {
            self.iter_loop.advance();
        }
        if let Some(reason) = end.stop_reason() {
            self.iter_loop.stop(reason);
        }
        if self.persist
            && let Err(error) = self.iter_loop.persist()
        {
            self.note(format!("could not checkpoint terminal state: {error}"));
        }
        if let LoopEnd::Failed(why) = &end {
            self.post(LoopEvent::Failed(why.clone()));
        }
        if end != LoopEnd::CockpitClosed {
            self.post(LoopEvent::Ended(end.to_string()));
        }
        let retain = self.keep_open
            && end != LoopEnd::CockpitClosed
            && !crate::utils::shutdown::Shutdown::is_requested();
        let report = LoopReport::new(self.iterations, end)
            .with_goal(self.goal_record())
            .with_metrics(std::mem::take(&mut self.metrics));
        if retain {
            let first = self.roadmap_request.take().map(Cmd::RoadmapPrompt);
            self.interact(first, false).await;
        }
        report
    }

    /// Follow-up sessions are outside the settled iteration: they cannot alter
    /// its report, spend accounting, gates, judge evidence, or attempt number.
    async fn interact(&mut self, first: Option<Cmd>, once: bool) -> bool {
        let config = self.agent_config("", &self.wf.project_dir);
        let session = crate::orchestration::interactive_session::session_with_checkpoints(
            config,
            self.connector.clone(),
            self.inbox.clone(),
            self.iter_loop
                .native()
                .cloned()
                .map(crate::application::workflow_client::EngineClient::attach),
            self.checkpoints.as_ref().map(|port| {
                Box::new(crate::orchestration::shared_checkpoint::SharedCheckpoint(
                    Arc::clone(port),
                )) as _
            }),
        );
        let exit = session.run(&mut self.commands, first, once).await;
        for control in exit.controls {
            self.decide(control);
        }
        // No start is taken here: a follow-up inside the work refuses one, and
        // the kept-open follow-up belongs to the terminal loop, whose model
        // offers none (a hosted session gets its commands back instead).
        exit.open
    }

    /// The boundary before an iteration: a skip or abort asked for
    /// meanwhile takes effect; a step that ran its one iteration pauses
    /// here and says so; a pause waits here for the user's decision.
    async fn boundary(&mut self) -> Boundary {
        if let Some(decision) = self.take_decision() {
            return decision;
        }
        if self.stepping {
            // The stepped iteration is behind us: hold, and say so the
            // way the auto-pause does, so the strip reads paused without
            // a second key from the user.
            self.stepping = false;
            self.paused = true;
            self.post(LoopEvent::Paused(format!(
                "stepped iteration {}",
                self.iter_loop.display_iteration().saturating_sub(1)
            )));
        }
        if !self.paused {
            return Boundary::Go;
        }
        self.note("paused — space resumes · s steps one iteration · x skips the next · esc aborts");
        loop {
            if let Some(text) = self.roadmap_request.take()
                && !self.interact(Some(Cmd::RoadmapPrompt(text)), true).await
            {
                return Boundary::Closed;
            }
            match self.wake(None, None).await {
                Wake::Cmd(Cmd::Loop(cmd)) => {
                    self.decide(cmd);
                    if !self.paused {
                        return self.take_decision().unwrap_or(Boundary::Go);
                    }
                }
                Wake::Closed => return Boundary::Closed,
                _ => {}
            }
        }
    }

    fn take_decision(&mut self) -> Option<Boundary> {
        match self.pending.take()? {
            LoopCmd::Skip => Some(Boundary::Skip),
            LoopCmd::Abort => Some(Boundary::Abort),
            LoopCmd::Pause | LoopCmd::Resume | LoopCmd::Step => None,
        }
    }

    /// One iteration: the stage cleared, the prompt built, the agent
    /// connected and run to the end of its turn, the gates run, the
    /// commit check recorded.
    async fn iteration(&mut self) -> Result<Step, String> {
        let (display, max) = (
            self.iter_loop.display_iteration(),
            self.iter_loop.max_iterations(),
        );
        let summary = self
            .iter_loop
            .spec_summary()
            .map_err(|e| format!("cannot read the roadmap's progress: {e}"))?;
        self.post(LoopEvent::Started {
            iteration: display,
            max,
        });
        self.note(format!(
            "iteration {display} of {max} — {}/{} done",
            summary.checked, summary.total
        ));
        if self.iterations == 0 {
            if let Some(note) = self.goal_note.take() {
                self.note(note);
            }
            self.note(self.goal_line());
        }
        self.transcript.clear();
        self.target = None;
        if let Some(engines) = self.iter_loop.native() {
            engines.reload_ship();
            engines.reload_signal();
            // The store re-read whole: the rail shows a plan — the Trace
            // tab the steps and gates, and the Roadmap tab's inbox the
            // signals — recorded while no feed was attached, without
            // waiting for a frame.
            self.inbox
                .send(Msg::Engine(EngineEvent::Seed(engines.snapshot())));
        }
        // Prompt, delegated goal and judge share the resumable selection.
        let target = self
            .iter_loop
            .next_native_chunk()
            .map_err(|e| format!("cannot choose the work to continue: {e}"))?;
        if let Some(chunk) = &target
            && chunk.status == ChunkStatus::InProgress
        {
            self.note(format!("Resuming {}", chunk.title));
        }
        self.target = target.map(|chunk| chunk.id);
        let prompt = self
            .iter_loop
            .next_prompt(&self.wf.project_config)
            .map_err(|e| format!("failed to build the prompt: {e}"))?;
        // What the last iteration's red gates measured, carried forward as
        // the rows themselves. Before the gates kept their output there was
        // nothing here to carry but a sentence.
        let prompt = match self.carry.take() {
            Some(brief) => format!("{prompt}\n\n{brief}"),
            None => prompt,
        };
        let cwd = self.wf.project_dir.clone();
        let started = Instant::now();
        let mut ticks = ticker();
        self.completed.clear();
        self.began = Some(chrono::Utc::now());
        self.split_part = None;
        self.verification_task = None;
        self.recorded_verification.clear();
        self.gates_unresolved = None;
        self.metrics.push(IterationMetrics::default());
        self.tokens_seen = None;
        let delegated = if self.delegating() {
            match self.delegated_turn(&cwd, started, &mut ticks).await? {
                Delegated::Ran => true,
                Delegated::Closed => return Ok(Step::Closed),
                Delegated::Fallback => false,
            }
        } else {
            false
        };
        if !delegated {
            // The dispatch, written before the launch. An outcome already
            // established for this attempt (recovery saw the runtime's
            // commit) means the provider's work is done: the gates and the
            // judge still run over it, no second runtime is launched.
            let dispatch = match self.intend(OperationKind::ProviderDispatch, "dispatch") {
                Intent::Attempt(attempt) => Some(attempt),
                Intent::Established => {
                    self.note(
                        "provider dispatch already established for this attempt — verifying its work instead of launching again",
                    );
                    None
                }
                Intent::Unrecorded(why) => {
                    self.iter_loop.mark_failed();
                    return Err(format!("not dispatching the agent: {why}"));
                }
            };
            if let Some(dispatch) = dispatch {
                let config = self.agent_config(&prompt, &cwd);
                self.post(LoopEvent::Stage(LoopStage::Connecting));
                let (transport, spec) = match self
                    .connector
                    .connect_checkpointed(config, || {
                        if self.persist {
                            self.iter_loop.checkpoint_dispatch_launch(&dispatch)
                        } else {
                            self.iter_loop.start_operation(&dispatch, None)
                        }
                    })
                    .await
                {
                    Ok(connected) => connected,
                    Err(e) => {
                        self.settled(&dispatch, OperationOutcome::Failed);
                        self.iter_loop.mark_failed();
                        return Err(format!("failed to start the agent: {e}"));
                    }
                };
                // The handle is on the record before anything else is
                // done with the runtime: a crash from here on leaves
                // recovery the particular process to ask about.
                self.launched(&dispatch, runtime_handle(&transport));
                let (session, session_tx, _) = match self.open_session(transport, spec, true) {
                    Ok(opened) => opened,
                    Err(why) => {
                        self.settled(&dispatch, OperationOutcome::Failed);
                        self.iter_loop.mark_failed();
                        return Err(why);
                    }
                };
                self.post(LoopEvent::Stage(LoopStage::Agent));
                let turn = self.turn(&session_tx, started, &mut ticks).await;
                drop(session_tx);
                self.close_session(session).await?;
                match turn {
                    Turn::Ended => self.settled(&dispatch, OperationOutcome::Completed),
                    Turn::Closed => {
                        self.settled(&dispatch, OperationOutcome::Cancelled);
                        return Ok(Step::Closed);
                    }
                    Turn::Failed(why) => {
                        self.settled(&dispatch, OperationOutcome::Failed);
                        self.iter_loop.mark_failed();
                        return Err(why);
                    }
                }
            }
        }
        if let Some(reason) = self.iter_loop.run_state().stop_reason() {
            self.iter_loop.stop(reason);
            return Err(format!("the run stopped before verification: {reason}"));
        }
        self.iterations += 1;
        let turn = started.elapsed();
        let tokens = self.tokens_seen.take();
        self.measure(|m| {
            m.turn = turn;
            m.tokens = tokens;
        });
        let gates_at = Instant::now();
        let Some(gates) = self.gates(started, &mut ticks).await else {
            return Ok(Step::Closed);
        };
        let gates_took = gates_at.elapsed();
        self.measure(|m| m.gates = gates_took);
        if !self.boundary_checkpoint(display, &gates).await {
            return Ok(Step::Closed);
        }
        if let Some(end) = self.settle(&gates) {
            return Ok(Step::End(end));
        }
        if delegated {
            self.note(
                "delegated iteration settled — the agent's own evaluator judged it; magistr saw no verdict",
            );
            return Ok(Step::Next);
        }
        Ok(self
            .judged(&gates, (display, max), started, &mut ticks)
            .await)
    }

    /// Drain one finished producer before another session can report cumulative cost.
    async fn close_session(
        &mut self,
        mut session: JoinHandle<Result<(), String>>,
    ) -> Result<(), String> {
        self.session_active = false;
        let cleanup = match tokio::time::timeout(SESSION_GRACE, &mut session).await {
            Ok(result) => result
                .map_err(|error| format!("the agent connection ended unexpectedly: {error}"))?,
            Err(_) => {
                session.abort();
                let _ = session.await;
                Err("the agent took too long to close and was stopped".to_owned())
            }
        };
        while let Ok(msg) = self.tap_rx.try_recv() {
            self.observe(&msg);
        }
        cleanup
    }

    fn accept_turn_end(&mut self, end: TurnEnd) -> Result<(), String> {
        let reason = match end {
            TurnEnd::EndTurn => return Ok(()),
            TurnEnd::Cancelled => magistr_engine::workflow::StopReason::Cancelled,
            _ => magistr_engine::workflow::StopReason::Failed,
        };
        self.iter_loop.stop(reason);
        Err(format!(
            "the agent stopped before completing its turn: {end:?}"
        ))
    }

    fn agent_config(&self, prompt: &str, cwd: &Path) -> AgentConfig {
        let mut config = AgentConfig::new(prompt, cwd)
            .backend(self.backend)
            .allowed_tools(ToolPermissions::for_workflow().into_strings())
            .workflow_permissions(true)
            .with_project_context(cwd);
        config.permission_profile = self.permission_profile;
        config
    }

    /// Open the next session using the same setup that connected its transport.
    fn open_session(
        &mut self,
        transport: magistr_acp::AcpTransport,
        spec: ResolvedSessionSpec,
        send_opening: bool,
    ) -> Result<OpenedSession, String> {
        self.iteration_cost_base = self.iter_loop.run_state().budget_total_usd;
        let (session_tx, session_commands) = Commands::channel();
        // The engine port acts on the loop's own in-process engines, so a
        // pin or checkpoint from the cockpit works mid-iteration too.
        let engine = self
            .iter_loop
            .native()
            .map(|engines| Box::new(NativeEnginePort::new(engines.clone())) as _);
        // The loop's cockpit reaches the same workbench the agent does:
        // the project's configured workbench-mcp, over its stdio.
        let workbench = crate::orchestration::commands::workbench_port::workbench_port(spec.cwd());
        let mut options = SessionOptions::new();
        if send_opening {
            options = options.opening_prompt(spec.prompt());
        }
        let (policy, _) = spec.into_permission_policy();
        let handle = CockpitSession::start_connected(transport, options, Box::new(policy))
            .map_err(|error| error.to_string())?;
        let control = handle.control();
        let session = spawn_session(
            Box::new(handle),
            self.inbox.tap(self.tap_tx.clone()),
            session_commands,
            workbench,
            engine,
            // The session holds no ledger of its own on the loop: every
            // checkpoint verb — the boundary's save, a `/checkpoint-now`,
            // a risky ask's `cp-N` — is served by the driver's port
            // instead ([`LoopDriver::ledger`]), off the session's relay,
            // and the boundary's is taken only once the session is gone.
            None,
        );
        self.session_active = true;
        Ok((session, session_tx, control))
    }

    /// The boundary checkpoint: the iteration's state, taken once its
    /// gates are in.
    ///
    /// The cockpit mints the row and asks for it back under that name;
    /// this task serves the ask on the port, off its own thread, and —
    /// alone among the port's callers — waits for the answer, because
    /// nothing else is going on: the iteration's session is gone and
    /// the next handshake has not begun. That ordering is the whole of
    /// the fix for flake, where one git read at `Started`
    /// sat on the delegated goal's echo deadline; here the two are
    /// sequential on one task, by construction. Both waits are bounded
    /// so a stalled runtime or a hung `git` cannot wedge the loop, and
    /// a late answer still lands on its row. `false` when the cockpit
    /// closed meanwhile.
    async fn boundary_checkpoint(&mut self, iteration: u32, gates: &[GateOutcome]) -> bool {
        if self.checkpoints.is_none() {
            return true;
        }
        self.post(LoopEvent::Boundary {
            iteration,
            gates: gates
                .iter()
                .map(|gate| (gate.name.clone(), gate.passed()))
                .chain(
                    self.recorded_verification
                        .iter()
                        .cloned()
                        .map(|name| (name, true)),
                )
                .collect(),
        });
        let asked = tokio::time::sleep(LEDGER_ASK_GRACE);
        tokio::pin!(asked);
        loop {
            tokio::select! {
                wake = self.wake(None, None) => match wake {
                    Wake::Ledger => break,
                    Wake::Closed => return false,
                    Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                    Wake::Tap(_) | Wake::Cmd(_) | Wake::Gate(_) | Wake::GatesGone | Wake::Tick => {}
                },
                () = &mut asked => {
                    self.note(format!(
                        "checkpoint for iteration {iteration}: the cockpit did not ask for it in time"
                    ));
                    return true;
                }
            }
        }
        if let Some(save) = self.ledger.take()
            && tokio::time::timeout(LEDGER_SAVE_GRACE, save).await.is_err()
        {
            self.note(format!(
                "checkpoint for iteration {iteration} is still saving — going on without it"
            ));
        }
        true
    }

    /// Serve one ledger verb on the port, off this task; the answer
    /// reaches the cockpit as `Msg::Checkpoint` when it lands.
    fn ledger(&mut self, cmd: CheckpointCmd) {
        if let Some(port) = &self.checkpoints {
            self.ledger = Some(detach(port, cmd, &self.inbox));
        }
    }

    /// One delegated turn: the objective compiled from the iteration's
    /// chunk, then a fresh session per candidate delivery — remembered
    /// across iterations once one confirms. Confirmation is the
    /// optimistic echo and nothing else (agent_goal.rs): it proves
    /// delivery and recognition, never acceptance or progress. No
    /// confirmation reports visibly and falls back to the native loop.
    async fn delegated_turn(
        &mut self,
        cwd: &Path,
        started: Instant,
        ticks: &mut Interval,
    ) -> Result<Delegated, String> {
        let chunk = self.iter_loop.native().and_then(|engines| {
            let id = self.target.clone()?;
            engines.roadmap_chunks().into_iter().find(|c| c.id == id)
        });
        let objective =
            AgentGoal::compile_objective(chunk.as_ref().map(|c| (c.id.as_str(), c.title.as_str())));
        let picks: Vec<DeliveryKind> = match self.delegated {
            Some(DelegationOutcome::Chosen(kind)) => vec![kind],
            Some(DelegationOutcome::NativeFallback) => Vec::new(),
            None => match self.goal {
                GoalMode::Extension => vec![DeliveryKind::Extension],
                GoalMode::Command => vec![DeliveryKind::Command],
                GoalMode::Auto => vec![DeliveryKind::Extension, DeliveryKind::Command],
                GoalMode::Native | GoalMode::Off => Vec::new(),
            },
        };
        for pick in picks {
            match self
                .goal_attempt(cwd, &objective, pick, started, ticks)
                .await?
            {
                Attempt::Confirmed(kind) => {
                    self.delegated = Some(DelegationOutcome::Chosen(kind));
                    return Ok(Delegated::Ran);
                }
                Attempt::Unavailable(why) => {
                    self.note(format!("goal {}: {why}", pick.label()));
                }
                Attempt::Unconfirmed(why) => self.post(LoopEvent::Failed(format!(
                    "goal {}: {why} — the delivery was cleared and abandoned",
                    pick.label()
                ))),
                Attempt::Closed => return Ok(Delegated::Closed),
            }
        }
        self.delegated = Some(DelegationOutcome::NativeFallback);
        self.post(LoopEvent::Failed(format!(
            "MAGISTR_GOAL={} asked to delegate the loop, but no delivery was confirmed by the agent's echo — running magistr's own loop instead",
            self.goal.label()
        )));
        Ok(Delegated::Fallback)
    }

    /// One delivery attempt on its own session, cleaned up whatever
    /// happened.
    async fn goal_attempt(
        &mut self,
        cwd: &Path,
        objective: &str,
        pick: DeliveryKind,
        started: Instant,
        ticks: &mut Interval,
    ) -> Result<Attempt, String> {
        let dispatch = match self.intend(
            OperationKind::ProviderDispatch,
            &format!("dispatch:{}", pick.label()),
        ) {
            Intent::Attempt(attempt) => attempt,
            Intent::Established => {
                return Ok(Attempt::Unavailable(
                    "this delivery is already established for the attempt".to_owned(),
                ));
            }
            Intent::Unrecorded(why) => {
                self.iter_loop.mark_failed();
                return Err(format!("not dispatching the agent: {why}"));
            }
        };
        let config = self.agent_config(objective, cwd);
        self.post(LoopEvent::Stage(LoopStage::Connecting));
        let (mut transport, spec) = match self
            .connector
            .connect_checkpointed(config, || {
                if self.persist {
                    self.iter_loop.checkpoint_dispatch_launch(&dispatch)
                } else {
                    self.iter_loop.start_operation(&dispatch, None)
                }
            })
            .await
        {
            Ok(connected) => connected,
            Err(e) => {
                self.settled(&dispatch, OperationOutcome::Failed);
                self.iter_loop.mark_failed();
                return Err(format!("failed to start the agent: {e}"));
            }
        };
        let Some(mut tap) = transport.tap_events() else {
            self.settled(&dispatch, OperationOutcome::Failed);
            return Err("the transport's event stream was already taken".to_owned());
        };
        self.launched(&dispatch, runtime_handle(&transport));
        let (session, session_tx, control) = match self.open_session(transport, spec, false) {
            Ok(opened) => opened,
            Err(why) => {
                self.settled(&dispatch, OperationOutcome::Failed);
                self.iter_loop.mark_failed();
                return Err(why);
            }
        };
        self.post(LoopEvent::Stage(LoopStage::Agent));
        let outcome = self
            .deliver(
                &control,
                &mut tap,
                &session_tx,
                objective,
                pick,
                started,
                ticks,
            )
            .await;
        drop(session_tx);
        self.close_session(session).await?;
        let settled = match &outcome {
            Ok(Attempt::Confirmed(_)) => OperationOutcome::Completed,
            Ok(Attempt::Unavailable(_) | Attempt::Closed) => OperationOutcome::Cancelled,
            Ok(Attempt::Unconfirmed(_)) | Err(_) => OperationOutcome::Failed,
        };
        self.settled(&dispatch, settled);
        outcome
    }

    /// Negotiate, issue, confirm by the echo, and run to the end of the
    /// agent's own goal loop.
    // The seven are the attempt's own session handles plus the loop's
    // clock — bundling them would be a struct for one call site.
    #[allow(clippy::too_many_arguments)]
    async fn deliver(
        &mut self,
        control: &SessionControl,
        tap: &mut UnboundedReceiver<AcpEvent>,
        session: &UnboundedSender<Cmd>,
        objective: &str,
        pick: DeliveryKind,
        started: Instant,
        ticks: &mut Interval,
    ) -> Result<Attempt, String> {
        // Phase 1 — the handshake, and for the command the advertised
        // list (it may land before or after `Initialized`; after gets
        // [`COMMANDS_GRACE`]).
        let mut delegation = Delegation::new();
        let mut application_ready = false;
        let negotiate = tokio::time::Instant::now() + NEGOTIATE_TIMEOUT;
        let mut grace: Option<tokio::time::Instant> = None;
        loop {
            if delegation.initialized()
                && application_ready
                && (pick == DeliveryKind::Extension || delegation.commands_listed())
            {
                break;
            }
            let deadline = match (grace, delegation.initialized()) {
                (Some(grace), _) => grace,
                (None, true) => {
                    let g = tokio::time::Instant::now() + COMMANDS_GRACE;
                    grace = Some(g);
                    g
                }
                (None, false) => negotiate,
            };
            tokio::select! {
                event = tap.recv() => match event {
                    Some(event) => delegation.observe(&event),
                    None => {
                        return Ok(Attempt::Unconfirmed(
                            "the session ended during negotiation".to_owned(),
                        ));
                    }
                },
                wake = self.wake(None, Some(ticks)) => match wake {
                    Wake::Closed => return Ok(Attempt::Closed),
                    Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                    Wake::Cmd(other) => {
                        let _ = session.send(other);
                    }
                    Wake::Tick => self.post(LoopEvent::Elapsed(started.elapsed())),
                    Wake::Tap(Msg::Agent(AgentEvent::Initialized { .. })) => {
                        application_ready = true;
                    }
                    Wake::Tap(Msg::Agent(AgentEvent::Error(e))) => {
                        return Err(format!("the agent failed: {e}"));
                    }
                    Wake::Tap(_) | Wake::Gate(_) | Wake::GatesGone | Wake::Ledger => {}
                },
                () = tokio::time::sleep_until(deadline) => break,
            }
        }
        if !application_ready {
            return Err("the supervised session did not finish initialization".to_owned());
        }
        let Some(delivery) = delegation.candidate_of_kind(pick) else {
            return Ok(Attempt::Unavailable(
                match pick {
                    DeliveryKind::Extension => "the agent advertises no usable goal extension",
                    DeliveryKind::Command => "the agent lists no goal command",
                }
                .to_owned(),
            ));
        };

        // Phase 2 — issue the delivery and wait for the optimistic echo.
        let mut pending: Option<oneshot::Receiver<magistr_acp::Result<serde_json::Value>>> = None;
        match &delivery {
            Delivery::Extension { method, session_id } => {
                self.note(format!("goal ({method}) → {objective}"));
                pending = Some(
                    control
                        .begin_goal(method.clone(), AgentGoal::set_params(session_id, objective))
                        .map_err(|e| e.to_string())?,
                );
            }
            Delivery::Command { name } => {
                let turn = AgentGoal::command_turn(name, objective);
                self.note(format!("goal → {turn}"));
                session.send(Cmd::Prompt(turn)).map_err(|e| e.to_string())?;
            }
        }
        let echo_deadline = tokio::time::Instant::now() + self.echo_timeout;
        let mut turn_ended = false;
        let confirmed = loop {
            tokio::select! {
                event = tap.recv() => match event {
                    Some(AcpEvent::SessionUpdate { update, .. })
                        if AgentGoal::echo(&update, objective) =>
                    {
                        break true;
                    }
                    Some(_) => {}
                    None => break false,
                },
                answer = extension_answer(&mut pending) => {
                    // Resolved before any echo: the goal turn ran with no
                    // recognition (or the method refused). The echo may
                    // still be a step behind the response — keep waiting.
                    let failed = answer.map(|result| result.is_err()).unwrap_or(true);
                    pending = None;
                    if failed {
                        break false;
                    }
                }
                wake = self.wake(None, Some(ticks)) => match wake {
                    Wake::Closed => return Ok(Attempt::Closed),
                    Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                    Wake::Cmd(other) => {
                        let _ = session.send(other);
                    }
                    Wake::Tick => self.post(LoopEvent::Elapsed(started.elapsed())),
                    Wake::Tap(Msg::Agent(AgentEvent::TurnEnded(end))) => {
                        self.accept_turn_end(end)?;
                        turn_ended = true;
                    }
                    Wake::Tap(Msg::Agent(AgentEvent::Error(e))) => {
                        return Err(format!("the agent failed: {e}"));
                    }
                    Wake::Tap(_) | Wake::Gate(_) | Wake::GatesGone | Wake::Ledger => {}
                },
                () = tokio::time::sleep_until(echo_deadline) => break false,
            }
        };
        if !confirmed {
            self.clear_delivery(control, session, &delivery, turn_ended, ticks)
                .await;
            return Ok(Attempt::Unconfirmed(format!(
                "no echo within {:.1} s",
                self.echo_timeout.as_secs_f64()
            )));
        }

        // Phase 3 — confirmed: the agent's own loop runs; magistr waits
        // for its end (the extension request resolving, or the command
        // turn's end) and stays blind in between.
        self.note(format!(
            "goal loop delegated ({}) — confirmed by the agent's echo (delivery and recognition, not progress); magistr is blind until it ends",
            delivery.label()
        ));
        let extension = matches!(delivery, Delivery::Extension { .. });
        // Both observations are required for extensions: the reply proves
        // execution succeeded, and the cockpit's terminal event proves all
        // preceding provider text crossed the presentation relay. Closing on
        // the reply alone can discard that relay's still-queued transcript.
        let mut tap = Some(tap);
        while !turn_ended || (extension && pending.is_some()) {
            tokio::select! {
                event = drained(&mut tap) => {
                    if event.is_none() {
                        return Err(
                            "the agent observation stream ended before the delegated turn settled"
                                .to_owned(),
                        );
                    }
                }
                answer = extension_answer(&mut pending) => {
                    pending = None;
                    answer
                        .map_err(|error| format!("the goal response was lost: {error}"))?
                        .map_err(|error| format!("the goal loop failed: {error}"))?;
                }
                wake = self.wake(None, Some(ticks)) => match wake {
                    Wake::Tap(Msg::Agent(AgentEvent::TurnEnded(end))) => {
                        self.accept_turn_end(end)?;
                        turn_ended = true;
                    }
                    Wake::Tap(Msg::Agent(AgentEvent::Error(e))) => {
                        return Err(format!("the agent failed: {e}"));
                    }
                    Wake::Closed => return Ok(Attempt::Closed),
                    Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                    Wake::Cmd(other) => {
                        let _ = session.send(other);
                    }
                    Wake::Tick => self.post(LoopEvent::Elapsed(started.elapsed())),
                    Wake::Tap(_) | Wake::Gate(_) | Wake::GatesGone | Wake::Ledger => {}
                },
            }
        }
        Ok(Attempt::Confirmed(pick))
    }

    /// Abandon a delivery that was issued but never confirmed: the goal
    /// — if the agent did set one — is cleared through the same
    /// vehicle, so falling back cannot leave two goals set.
    async fn clear_delivery(
        &mut self,
        control: &SessionControl,
        session: &UnboundedSender<Cmd>,
        delivery: &Delivery,
        turn_ended: bool,
        ticks: &mut Interval,
    ) {
        match delivery {
            Delivery::Extension { method, session_id } => {
                let _ = control.clear_goal(method.clone(), AgentGoal::clear_params(session_id));
            }
            Delivery::Command { name } => {
                if !turn_ended {
                    let _ = session.send(Cmd::Cancel);
                    // The transport drops a prompt while one is in
                    // flight, so wait for the turn's end first.
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
                    loop {
                        tokio::select! {
                            wake = self.wake(None, Some(ticks)) => match wake {
                                Wake::Tap(Msg::Agent(AgentEvent::TurnEnded(_)))
                                | Wake::Closed => break,
                                _ => {}
                            },
                            () = tokio::time::sleep_until(deadline) => break,
                        }
                    }
                }
                let _ = session.send(Cmd::Prompt(AgentGoal::command_clear_turn(name)));
            }
        }
        tokio::time::sleep(CLEAR_GRACE).await;
    }

    /// The native goal loop's half of an iteration: the target chunk
    /// re-read after the agent's own process wrote the store, the judge
    /// asked on the driver's own wake loop (so a quit, a pause and the
    /// clock all still land while it thinks), the verdict reported, and
    /// the reconciled decision carried out through the engines.
    async fn judged(
        &mut self,
        gates: &[GateOutcome],
        iteration: (u32, u32),
        started: Instant,
        ticks: &mut Interval,
    ) -> Step {
        let Some(judge) = self.judge.clone() else {
            return Step::Next;
        };
        let Some(engines) = self.iter_loop.native().cloned() else {
            return Step::Next;
        };
        engines.reload_roadmap();
        let Some(chunk) = self.target_chunk(&engines) else {
            self.note("judge not asked: no chunk to judge this iteration");
            return Step::Next;
        };
        // A chunk the agent finished, blocked or retired during its turn
        // is not a question for the judge: the only verdict on a chunk
        // nobody could have worked is "not worked", and it costs a
        // judge call to hear it. The next iteration's target is
        // roadmap_next's answer, which skips all three. A blocker is
        // read off `blocked_by`, the field roadmap_next skips on — the
        // roadmap's `set_blocked_by` records the reason and leaves the
        // status where it was, so `ChunkStatus::Blocked` alone would
        // miss exactly the agent that did the right thing.
        if let Some(blocker) = &chunk.blocked_by {
            self.note(format!(
                "judge not asked: {} was blocked by the agent this iteration — {}",
                chunk.id, blocker.reason
            ));
            return Step::Next;
        }
        if let Some(part) = &self.split_part
            && chunk.status != ChunkStatus::Done
        {
            self.note(format!(
                "judge not asked: the agent split {} and shipped {part} this iteration; the rest is still to do",
                chunk.id
            ));
            return Step::Next;
        }
        match chunk.status {
            ChunkStatus::Done => {
                self.note(format!(
                    "judge not asked: {} was completed by the agent this iteration",
                    chunk.id
                ));
                return Step::Next;
            }
            ChunkStatus::Blocked => {
                self.note(format!(
                    "judge not asked: {} was blocked by the agent this iteration",
                    chunk.id
                ));
                return Step::Next;
            }
            ChunkStatus::Obsoleted => {
                self.note(format!(
                    "judge not asked: {} was obsoleted by the agent this iteration",
                    chunk.id
                ));
                return Step::Next;
            }
            ChunkStatus::Backlog | ChunkStatus::Pending | ChunkStatus::InProgress => {}
        }
        let evidence: Vec<GateEvidence> = gates.iter().map(GateEvidence::from).collect();
        let input = JudgeInput {
            chunk: chunk.id.clone(),
            title: chunk.title.clone(),
            acceptance: chunk.acceptance.clone(),
            gates: evidence.clone(),
            recorded_checks: self.recorded_verification.clone(),
            transcript_tail: std::mem::take(&mut self.transcript),
            iteration,
        };
        self.post(LoopEvent::Stage(LoopStage::Judge));
        self.note(format!("asking the judge about {}", chunk.id));
        let asked = Instant::now();
        let mut ask = Box::pin(async move { judge.judge(&input).await });
        let verdict = loop {
            tokio::select! {
                verdict = &mut ask => break verdict,
                wake = self.wake(None, Some(ticks)) => match wake {
                    Wake::Closed => return Step::Closed,
                    Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                    Wake::Tick => self.post(LoopEvent::Elapsed(started.elapsed())),
                    Wake::Tap(_)
                    | Wake::Cmd(_)
                    | Wake::Gate(_)
                    | Wake::GatesGone
                    | Wake::Ledger => {}
                },
            }
        };
        let judge_took = asked.elapsed();
        self.measure(|m| m.judge = Some(judge_took));
        self.post(LoopEvent::Verdict {
            iteration: iteration.0,
            ok: verdict.ok,
            impossible: verdict.impossible,
            reason: verdict.reason.clone(),
        });
        match Decision::reconcile(&verdict, &evidence) {
            Decision::Complete { reason } => {
                self.complete(&engines, &chunk, &reason);
                Step::Next
            }
            Decision::Continue { .. } => {
                // The reason is already on the stage: the Verdict event
                // above put it there, so the note only says what follows.
                self.note(format!(
                    "judge: not yet — the next iteration takes {} again",
                    chunk.id
                ));
                Step::Next
            }
            Decision::Disagreement { reason, failed } => {
                self.post(LoopEvent::Failed(format!(
                    "the judge says {} is met ({reason}) but the gates disagree: {} failed — the chunk stays in progress",
                    chunk.id,
                    failed.join(", ")
                )));
                Step::Next
            }
            Decision::Stop { reason } => self.stop(&engines, &chunk, &reason),
        }
    }

    /// Whether `objective` is the work this iteration did. The agent owns
    /// that answer, not the prompt: it may ship the item it was given, or
    /// split an item too large for one change and ship a part (`/next` tells
    /// it to). Three witnesses, any of which is enough: the objective names
    /// the item the prompt was built for (which also covers work resumed
    /// from an earlier iteration), it names an item the feed saw completed
    /// this iteration, or it was set after this iteration began for a roadmap
    /// item the agent started or finished. An objective left over from
    /// earlier work, or one that names no roadmap item, is none of these.
    fn is_iteration_work(
        &self,
        engines: &NativeEngines,
        objective: &magistr_workflow::ship::domain::objective::Objective,
    ) -> bool {
        let named = objective.scope.strip_prefix("chunk:");
        if named.is_some() && named == self.target.as_deref() {
            return true;
        }
        if named.is_some_and(|id| self.completed.iter().any(|done| done == id)) {
            return true;
        }
        let set_at = objective
            .created_at
            .as_deref()
            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok());
        let worked = named
            .and_then(|id| engines.roadmap_chunk(id))
            .is_some_and(|chunk| {
                matches!(chunk.status, ChunkStatus::Done | ChunkStatus::InProgress)
            });
        worked && matches!((set_at, self.began), (Some(set_at), Some(began)) if set_at >= began)
    }

    /// Judge the work selected for this iteration, even if another item is
    /// in progress. An unselected legacy iteration may use its single active item.
    fn target_chunk(&self, engines: &NativeEngines) -> Option<Chunk> {
        let chunks = engines.roadmap_chunks();
        if let Some(target) = &self.target {
            return chunks.into_iter().find(|chunk| &chunk.id == target);
        }
        let mut active = chunks
            .into_iter()
            .filter(|chunk| chunk.status == ChunkStatus::InProgress);
        let chunk = active.next()?;
        active.next().is_none().then_some(chunk)
    }

    /// Complete `chunk` through the engines (starting one the agent
    /// never started, since the lifecycle refuses pending → done) and
    /// put the completion frame in front of the cockpit.
    fn complete(&mut self, engines: &NativeEngines, chunk: &Chunk, reason: &str) {
        if chunk.status == ChunkStatus::Pending {
            let started = self.mutate(&format!("start_chunk:{}", chunk.id), || {
                engines.roadmap_start_chunk(&chunk.id).map(|_| ())
            });
            if let Err(e) = started {
                self.post(LoopEvent::Failed(format!(
                    "the judge says {} is met but the engines refused to start it: {e}",
                    chunk.id
                )));
                return;
            }
        }
        // The proof-of-ship reference: the task this iteration's gates were
        // recorded on. A completion with no such task carries none, and
        // says so in the record rather than inventing one.
        let ship_ref = self
            .verification_task
            .as_ref()
            .map(|task| format!("task:{task}"));
        let completed = self.mutate(&format!("complete_chunk:{}", chunk.id), || {
            engines.roadmap_complete_chunk(&chunk.id, ship_ref.as_deref())
        });
        match completed {
            Ok(Some(done)) => {
                self.completed.push(done.id.clone());
                self.post_chunk_event(RoadmapFrame::ChunkCompleted { chunk: done });
                self.note(format!("judge: met — {reason}; {} completed", chunk.id));
            }
            Ok(None) => self.note(format!(
                "judge: met — {reason}; {} was already completed for this attempt",
                chunk.id
            )),
            Err(e) => self.post(LoopEvent::Failed(format!(
                "the judge says {} is met but the engines refused to complete it: {e}",
                chunk.id
            ))),
        }
    }

    /// One native mutation, its intent written before the engines are
    /// asked and its outcome settled by their answer. `Ok(None)` when the
    /// mutation is already established for this attempt and not redone;
    /// an intent that cannot be written is not dispatched.
    fn mutate<T>(
        &mut self,
        target: &str,
        apply: impl FnOnce() -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        let attempt = match self.intend(OperationKind::NativeMutation, target) {
            Intent::Attempt(attempt) => attempt,
            Intent::Established => return Ok(None),
            Intent::Unrecorded(why) => return Err(format!("not applied: {why}")),
        };
        let applied = apply();
        self.settled(
            &attempt,
            if applied.is_ok() {
                OperationOutcome::Completed
            } else {
                OperationOutcome::Failed
            },
        );
        applied.map(Some)
    }

    /// Write an operation's intent for this attempt, durably when the
    /// loop persists. The key doubles as the target: what recovery reads
    /// back is spelled the same way the record names it.
    fn intend(&mut self, kind: OperationKind, target: &str) -> Intent {
        match self.iter_loop.begin_operation(kind, target, target) {
            Ok(Some(attempt)) => {
                if self.persist
                    && let Err(error) = self.iter_loop.persist()
                {
                    return Intent::Unrecorded(error.to_string());
                }
                Intent::Attempt(attempt)
            }
            Ok(None) => Intent::Established,
            Err(error) => Intent::Unrecorded(error.to_string()),
        }
    }

    /// The effect is dispatched; `runtime` is its own handle.
    fn launched(&mut self, attempt: &AttemptId, runtime: Option<String>) {
        if let Err(error) = self.iter_loop.start_operation(attempt, runtime) {
            self.note(format!("operation not marked running: {error}"));
        }
        self.checkpoint_operations();
    }

    /// The effect's outcome, as this owner saw it.
    fn settled(&mut self, attempt: &AttemptId, outcome: OperationOutcome) {
        if let Err(error) = self.iter_loop.settle_operation(attempt, outcome) {
            self.note(format!("operation not settled: {error}"));
        }
        self.checkpoint_operations();
    }

    /// The checkpoint that makes a record durable — before the effect it
    /// names, and after its outcome.
    fn checkpoint_operations(&mut self) {
        if self.persist
            && let Err(error) = self.iter_loop.persist()
        {
            self.note(format!(
                "could not checkpoint the operation record: {error}"
            ));
        }
    }

    /// Block `chunk` on a human with the judge's reason — the dock's
    /// blocker card — and end the loop.
    fn stop(&mut self, engines: &NativeEngines, chunk: &Chunk, reason: &str) -> Step {
        let blocked = self.mutate(&format!("set_blocked_by:{}", chunk.id), || {
            engines.roadmap_set_blocked_by(
                &chunk.id,
                BlockerKind::AwaitingHuman,
                format!("the judge stopped the loop: {reason}"),
                None,
            )
        });
        match blocked {
            Ok(Some(blocked)) => {
                self.post_chunk_event(RoadmapFrame::ChunkChanged { chunk: blocked })
            }
            Ok(None) => {}
            Err(e) => self.post(LoopEvent::Failed(format!(
                "could not block {} on a human: {e}",
                chunk.id
            ))),
        }
        Step::End(LoopEnd::NeedsHuman(reason.to_owned()))
    }

    /// A roadmap mutation of the driver's own, as the typed event the
    /// engines would have announced — the rail, the dock and the Roadmap
    /// tab fold it through the one path they already have; the in-process
    /// engines write no socket, so the event is this process's own.
    ///
    /// A publication is recorded like any other operation, and is the one
    /// kind recovery never redoes: a surface that missed the event reseeds
    /// from the store, which is where the mutation it announces already is.
    fn post_chunk_event(&mut self, frame: RoadmapFrame) {
        let (kind, id) = match &frame {
            RoadmapFrame::ChunkChanged { chunk } => ("chunk_changed", chunk.id.clone()),
            RoadmapFrame::ChunkCompleted { chunk } => ("chunk_completed", chunk.id.clone()),
            RoadmapFrame::ChunkAdded { chunk } => ("chunk_added", chunk.id.clone()),
            RoadmapFrame::ChunkObsoleted { chunk } => ("chunk_obsoleted", chunk.id.clone()),
            RoadmapFrame::RefreshRecorded { .. } => ("refresh_recorded", String::new()),
        };
        let publication =
            match self.intend(OperationKind::Publication, &format!("frame:{kind}:{id}")) {
                Intent::Attempt(attempt) => Some(attempt),
                Intent::Established => return,
                Intent::Unrecorded(_) => None,
            };
        self.inbox
            .send(Msg::Engine(EngineEvent::local(NativeEvent::Roadmap(frame))));
        if let Some(publication) = publication {
            self.settled(&publication, OperationOutcome::Completed);
        }
    }

    /// The agent's turn: the cockpit's commands forwarded to the session,
    /// the clock kept, until the turn ends.
    async fn turn(
        &mut self,
        session: &UnboundedSender<Cmd>,
        started: Instant,
        ticks: &mut Interval,
    ) -> Turn {
        loop {
            match self.wake(None, Some(ticks)).await {
                Wake::Tap(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn))) => {
                    return Turn::Ended;
                }
                Wake::Tap(Msg::Agent(AgentEvent::TurnEnded(end))) => {
                    return Turn::Failed(self.accept_turn_end(end).expect_err("non-complete turn"));
                }
                Wake::Tap(Msg::Agent(AgentEvent::Error(e))) => {
                    return Turn::Failed(format!("the agent failed: {e}"));
                }
                Wake::Tap(Msg::Agent(AgentEvent::PromptRejected { error, .. })) => {
                    return Turn::Failed(format!("The message could not be sent: {error}"));
                }
                Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                Wake::Cmd(other) => {
                    let _ = session.send(other);
                }
                Wake::Tick => self.post(LoopEvent::Elapsed(started.elapsed())),
                Wake::Closed => return Turn::Closed,
                Wake::Tap(_) | Wake::Gate(_) | Wake::GatesGone | Wake::Ledger => {}
            }
        }
    }

    /// The gates as jobs, every one announced on the strip; `None` when
    /// the cockpit closed meanwhile — cancelling the jobs on the way
    /// out, so no gate's process outlives the quit.
    ///
    /// The set is the one native completion verifies: the contracts the
    /// project's evidence capability resolves for its root at this moment
    /// — one spec per archetype command in the archetype's directory, the
    /// project's own command once however many archetypes name it, and
    /// the required custom gates — under the names and commands the
    /// contracts carry, so a judged completion stands on these receipts.
    /// A set that cannot be resolved fails the iteration at the boundary:
    /// completion would refuse the same unreadable configuration, and an
    /// empty green set would say "nothing configured" about a project
    /// that configured something.
    ///
    /// One at a time. `cargo check`, `cargo nextest` and `cargo clippy`
    /// contend for one build lock on one target directory, so racing them
    /// buys no wall clock and costs the ability to say which gate spent
    /// which second. A red required gate gets exactly one more run: green
    /// on the retry is a flake, named rather than hidden, and not a reason
    /// to stop the loop; red twice is real.
    async fn gates(&mut self, started: Instant, ticks: &mut Interval) -> Option<Vec<GateOutcome>> {
        // The agent is gone and its feed with it (the socket is the
        // agent's own mcp-serve's): the stage is what the cockpit shows
        // in its place.
        self.post(LoopEvent::Stage(LoopStage::Gates));
        let cancel = CancellationToken::new();
        // The trace the gates are recorded into was written by the agent's
        // own `magistr mcp-serve`, in another process; persistence is
        // merge-on-save and never reload-on-read, so this handle cannot see
        // that task until it re-reads. Once per set, not once per gate.
        let engines = self.iter_loop.native().cloned();
        if let Some(engines) = &engines {
            engines.reload_ship();
        }
        // Focused work owns its chosen checks. A finalized objective has
        // already closed its recording task. In both cases validate the
        // existing receipts through the same authority as completion;
        // never expand to the whole suite or run checks without a task.
        if let Some(engines) = &engines
            && let Some(objective) = engines.ship_objective()
            && (objective.verification
                == magistr_workflow::ship::domain::objective::VerificationMode::Focused
                || objective.status
                    == magistr_workflow::ship::domain::objective::ObjectiveStatus::Completed)
        {
            engines.reload_roadmap();
            if !self.is_iteration_work(engines, &objective) {
                self.gates_unresolved = Some(
                    "The recorded checks belong to different work. Restore this item's verification task before continuing.".to_owned(),
                );
                return Some(Vec::new());
            }
            match engines.ship_verify_completion(&objective.scope, None) {
                Ok(evidence) if !evidence.accepted.is_empty() => {
                    self.split_part = objective
                        .scope
                        .strip_prefix("chunk:")
                        .filter(|part| self.target.as_deref() != Some(*part))
                        .map(str::to_owned);
                    self.verification_task = Some(evidence.task_id);
                    self.recorded_verification = evidence.accepted.into_iter()
                        .map(|receipt| receipt.gate).collect();
                }
                Ok(_) => self.gates_unresolved = Some(
                    "No passing checks are recorded for this work. Run and record the relevant checks before continuing.".to_owned(),
                ),
                Err(why) => self.gates_unresolved = Some(format!(
                    "The recorded checks no longer verify this work. Review and rerun the affected checks: {why}"
                )),
            }
            return Some(Vec::new());
        }
        // Named once for the set, before any gate runs: every record of
        // this set lands on this task, and the completion names it.
        let verification = engines
            .as_ref()
            .ok_or_else(|| "no think-and-ship engines".to_owned())
            .and_then(|engines| engines.ship_admit_task(None));
        self.verification_task = verification
            .as_ref()
            .ok()
            .map(|target| target.task_id.clone());
        let mut said_why_not = false;
        // The same capability, on the same root, read the same way the
        // completion reads it: the admitted writer's own evidence port; a
        // loop without engines asks the one resolver behind that port and
        // runs the identical set unrecorded.
        let resolved = match engines
            .as_ref()
            .and_then(|engines| engines.authority().config().evidence())
        {
            Some(port) => port.required_gates(),
            None => WorkspaceEvidence::gate_contracts(&self.wf.project_dir),
        };
        let contracts = match resolved {
            Ok(contracts) => contracts,
            Err(why) => {
                self.gates_unresolved = Some(why);
                return Some(Vec::new());
            }
        };
        let mut outcomes = Vec::new();
        for contract in contracts {
            let (name, command) = (contract.name, contract.command);
            if command.trim().is_empty() {
                continue;
            }
            // Intended, then admitted, before the run: the record names
            // the receipt it will settle, and the receipt names the source
            // as it is now, through the evidence capability the host
            // writer was admitted with.
            let admission = match &verification {
                Ok(target) => self.intend_receipt(
                    engines.as_ref().expect("verification has an owner"),
                    target,
                    &name,
                    &command,
                ),
                Err(why) => Err(why.clone()),
            };
            let first = match self
                .one_gate(&name, &command, &cancel, started, ticks)
                .await
            {
                GateStep::Ran(outcome) => outcome,
                GateStep::Gone => continue,
                GateStep::Closed => {
                    cancel.cancel();
                    return None;
                }
            };
            if !first.is_retryable() {
                self.record_gate(engines.as_ref(), &admission, &first, &mut said_why_not);
                outcomes.push(first);
                continue;
            }
            self.note(format!(
                "{} failed (exit {}) — running it once more to tell a flake from a failure",
                first.name, first.exit_code
            ));
            match self
                .one_gate(&name, &command, &cancel, started, ticks)
                .await
            {
                GateStep::Ran(again) if again.passed() => {
                    let flake = again.as_flake_of(&first);
                    self.record_gate(engines.as_ref(), &admission, &flake, &mut said_why_not);
                    outcomes.push(flake);
                }
                // Red twice is real, and the second run's own evidence is
                // the fresher of the two.
                GateStep::Ran(again) => {
                    self.record_gate(engines.as_ref(), &admission, &again, &mut said_why_not);
                    outcomes.push(again);
                }
                GateStep::Gone => {
                    self.record_gate(engines.as_ref(), &admission, &first, &mut said_why_not);
                    outcomes.push(first);
                }
                GateStep::Closed => {
                    cancel.cancel();
                    return None;
                }
            }
        }
        Some(outcomes)
    }

    /// The record of one gate's receipt, written before the gate is
    /// admitted, and the admission itself once it is: `Recording.receipt`
    /// is the attempt whose handle names the durable receipt, `None` when
    /// the intent could not be written (the gate is recorded anyway — its
    /// evidence is worth more than the record of recording it). An
    /// `Established` intent means this attempt's receipt for the gate is
    /// already applied — recovery found it settled — and nothing is
    /// admitted: a second admission would leave an unsettled latest
    /// receipt in front of the applied one, and completion would refuse
    /// on it. An admission that fails settles the intent `Failed` so the
    /// next set admits attempt two.
    fn intend_receipt(
        &mut self,
        engines: &NativeEngines,
        target: &magistr_engine::native::TaskAdmission,
        name: &str,
        command: &str,
    ) -> Result<Recording, String> {
        let task = target.task_id.clone();
        let receipt = match self.intend(OperationKind::Receipt, &format!("gate:{name}@task:{task}"))
        {
            Intent::Attempt(attempt) => Some(attempt),
            Intent::Established => {
                return Err(format!(
                    "{name} already recorded on {task} for this attempt — not recorded twice"
                ));
            }
            Intent::Unrecorded(_) => None,
        };
        let admitted = AdmittedCheck::admit(
            engines,
            &self.wf.project_dir,
            Some(&task),
            name,
            command.to_owned(),
            true,
        )
        .map_err(|why| why.to_string())
        .and_then(|admission| {
            if admission.target() == target {
                Ok(admission)
            } else {
                Err("the verification run or task was superseded".to_owned())
            }
        });
        match admitted {
            Ok(admission) => {
                if let Some(receipt) = &receipt {
                    self.launched(receipt, Some(format!("receipt:{}", admission.receipt_id())));
                }
                Ok(Recording { admission, receipt })
            }
            Err(why) => {
                if let Some(receipt) = &receipt {
                    self.settled(receipt, OperationOutcome::Failed);
                }
                Err(why)
            }
        }
    }

    /// Forward one gate to the execution trace as a verified check, with
    /// its real exit code, its command and its captured output — the same
    /// record `magistr_gate` writes for the agent's own gates, and which
    /// the driver's had no counterpart to at all: it ran three cargos
    /// between every iteration and wrote nothing about any of them.
    ///
    /// A `cargo nextest` whose profile writes a junit report has it parsed
    /// into the record additively; the exit code decides `passed`, never
    /// the report, and a missing file is "not parsed" rather than a
    /// failure. `said` keeps a set that cannot be recorded to one note on
    /// the stage instead of one per gate.
    fn record_gate(
        &mut self,
        engines: Option<&NativeEngines>,
        recording: &Result<Recording, String>,
        gate: &GateOutcome,
        said: &mut bool,
    ) {
        let Some(engines) = engines else {
            if !*said {
                *said = true;
                self.note("gates not recorded: no workflow engines");
            }
            return;
        };
        let Recording { admission, receipt } = match recording {
            Ok(recording) => recording.clone(),
            Err(why) => {
                if !*said {
                    *said = true;
                    self.note(format!("gates not recorded: {why}"));
                }
                return;
            }
        };
        // The boundary derives the junit report path under the project
        // directory itself; the loop makes no promise about its own
        // working directory.
        let recorded = admission
            .settle(engines, gate.execution(), gate.details())
            .map_err(|why| why.to_string())
            .and_then(|settled| match settled.settlement {
                CheckSettlement::Applied { check, .. } => Ok(check),
                CheckSettlement::Stale { admission, .. } => Err(format!(
                    "check belongs to superseded run {} task {}; result remains in the loop gate outcome",
                    admission.target.run_id, admission.target.task_id,
                )),
            });
        if let Some(receipt) = receipt {
            self.settled(
                &receipt,
                if recorded.is_ok() {
                    OperationOutcome::Completed
                } else {
                    OperationOutcome::Failed
                },
            );
        }
        if let Err(why) = recorded
            && !*said
        {
            *said = true;
            self.note(format!("gates not recorded: {why}"));
        }
    }

    /// One gate as a job, awaited to its outcome on a channel of its own
    /// — so a job that vanishes without reporting is still detected per
    /// gate, the way one shared channel could only detect it for all.
    async fn one_gate(
        &mut self,
        name: &str,
        command: &str,
        cancel: &CancellationToken,
        started: Instant,
        ticks: &mut Interval,
    ) -> GateStep {
        let (report, mut outcomes_rx) = unbounded_channel();
        // The gate's own way out: a child of the driver's token, so the
        // cockpit closing still ends it, and the held kill ends this gate alone — its process group, the way a
        // timeout would — and the run comes back cancelled.
        let own = cancel.child_token();
        let job = self.jobs.spawn_job(gate_job(
            name.to_owned(),
            command.to_owned(),
            self.wf.project_dir.clone(),
            report,
            self.inbox.clone(),
            own.clone(),
        ));
        self.post(LoopEvent::GateStarted {
            name: name.to_owned(),
            job,
        });
        loop {
            match self.wake(Some(&mut outcomes_rx), Some(ticks)).await {
                Wake::Gate(outcome) => return GateStep::Ran(outcome),
                Wake::GatesGone => return GateStep::Gone,
                Wake::Cmd(Cmd::Loop(cmd)) => self.decide(cmd),
                Wake::Cmd(Cmd::Kill(KillCmd::Gate(gate))) if gate == name => own.cancel(),
                Wake::Tick => self.post(LoopEvent::Elapsed(started.elapsed())),
                Wake::Closed => return GateStep::Closed,
                Wake::Tap(_) | Wake::Cmd(_) | Wake::Ledger => {}
            }
        }
    }

    /// After the gates: the commit check, the chunks the feed saw
    /// completed, the gates' verdict and the auto-pause; the reason to
    /// stop, if there is one.
    fn settle(&mut self, gates: &[GateOutcome]) -> Option<LoopEnd> {
        match self.iter_loop.post_iteration_check() {
            PostCheckResult::CommitMade { new_sha } => {
                let short = new_sha.get(..7).unwrap_or(new_sha.as_str());
                self.note(format!("committed {short}"));
            }
            PostCheckResult::NoCommit { guidance } => {
                self.note(format!("no commit this iteration — {guidance}"));
                if self.iter_loop.should_stop_no_commits() {
                    return Some(LoopEnd::NoCommits);
                }
            }
        }
        if self.iter_loop.is_native() {
            if self.completed.is_empty() {
                self.note("no roadmap chunk was completed this iteration");
            } else {
                self.note(format!("completed chunk(s): {}", self.completed.join(", ")));
            }
        }
        let flaky: Vec<&str> = gates
            .iter()
            .filter(|gate| gate.verdict == GateVerdict::Flaky)
            .map(|gate| gate.name.as_str())
            .collect();
        if !flaky.is_empty() {
            self.note(format!(
                "flaky, passed on the retry: {} — not treated as a failure",
                flaky.join(", ")
            ));
        }
        let failed: Vec<String> = gates
            .iter()
            .filter(|gate| !gate.passed())
            .map(|gate| format!("{} {}", gate.name, gate.verdict.label()))
            .collect();
        if let Some(why) = self.gates_unresolved.take() {
            self.post(LoopEvent::Failed(format!(
                "quality gates not resolved — nothing ran: {why}"
            )));
            if self.auto_pause {
                self.paused = true;
                self.post(LoopEvent::Paused(format!(
                    "quality gates not resolved: {why}"
                )));
            }
            return Some(LoopEnd::Failed(why));
        } else if !self.recorded_verification.is_empty() {
            self.note(format!(
                "Recorded checks still verify this work: {}.",
                self.recorded_verification.join(", ")
            ));
        } else if gates.is_empty() {
            self.note("no quality gates configured");
        } else if failed.is_empty() {
            let secs: f64 = gates.iter().map(|g| g.duration.as_secs_f64()).sum();
            self.note(format!("quality gates passed in {secs:.1}s"));
        } else {
            let names = failed.join(", ");
            self.post(LoopEvent::Failed(format!("quality gates failed: {names}")));
            if self.auto_pause {
                self.paused = true;
                self.post(LoopEvent::Paused(format!("quality gates failed: {names}")));
            }
            self.verify(gates);
        }
        None
    }

    /// The gates as the independent verifier of the agent's own claim.
    ///
    /// They run after the agent has already run its own gates, completed
    /// its chunk and committed, so they can prevent none of it — the
    /// commit boundary is the inner/outer-loop split and the pre-commit
    /// position is already held by the agent's `magistr_gate`. Their
    /// authority is over the ROADMAP instead: a chunk the
    /// agent completed itself while a required gate is red does not stay
    /// done. Nothing here reaches git; no commit is reverted, reset or
    /// stashed. `Done -> InProgress` is the roadmap's own legal
    /// transition, the one its lifecycle table names `reopen`.
    ///
    /// Reopening is also what puts the chunk back in front of
    /// [`Decision::reconcile`](crate::orchestration::goal::Decision):
    /// `judged` skips a chunk that is `Done`, which is exactly what the
    /// `/next` workflow tells the agent to leave behind, so the authority
    /// rule could never run on the real path. It runs now.
    ///
    /// Three kinds of red are deliberately not evidence of anything and
    /// reopen nothing: a flake (it passed), a cancelled gate (the cockpit
    /// is closing and nobody let it finish), and a gate the project
    /// registered not-required (it is expected to fail).
    fn verify(&mut self, gates: &[GateOutcome]) {
        let red: Vec<&GateOutcome> = gates
            .iter()
            .filter(|gate| gate.required && !gate.passed() && gate.verdict.is_evidence())
            .collect();
        if red.is_empty() {
            return;
        }
        self.carry = Some(failure_brief(&red));
        let Some(engines) = self.iter_loop.native().cloned() else {
            return;
        };
        engines.reload_roadmap();
        // Two witnesses to the same fact, because either can be the only
        // one present. The feed's `chunk_completed` frames are what the
        // driver saw this iteration; the chunk the prompt was built for,
        // now reading Done in the store the agent's own `magistr
        // mcp-serve` wrote, is the route `judged` already trusts.
        let mut ids = self.completed.clone();
        if let Some(target) = self.target.clone()
            && !ids.contains(&target)
        {
            ids.push(target);
        }
        let names: Vec<&str> = red.iter().map(|gate| gate.name.as_str()).collect();
        let names = names.join(", ");
        for id in ids {
            // Only a chunk that is actually done gets reopened: nothing
            // here may move a chunk that was merely started.
            if engines.roadmap_chunk(&id).map(|chunk| chunk.status) != Some(ChunkStatus::Done) {
                continue;
            }
            let reopened = self.mutate(&format!("set_status:{id}:in_progress"), || {
                engines.roadmap_set_status(&id, ChunkStatus::InProgress)
            });
            match reopened {
                Ok(Some(chunk)) => {
                    self.note(format!("{id} reopened: {names} is red"));
                    self.post_chunk_event(RoadmapFrame::ChunkChanged { chunk });
                }
                Ok(None) => {}
                Err(e) => self.note(format!("could not reopen {id}: {e}")),
            }
        }
    }

    /// Advance the loop, persist when asked, clear the workflow's files.
    fn advance(&mut self) {
        self.iter_loop.advance();
        if self.persist
            && let Err(e) = self.iter_loop.persist()
        {
            self.note(format!("could not persist the loop's state: {e}"));
        }
        let _ = self.wf.cleanup();
    }

    /// A loop decision from the cockpit: pauses and resumes take effect
    /// as flags, a step arms the pause for the boundary after the
    /// iteration it lets run, a skip or an abort waits for the boundary.
    /// Every decision but Step ends a step in flight — the newest word
    /// wins.
    fn decide(&mut self, cmd: LoopCmd) {
        self.stepping = false;
        match cmd {
            LoopCmd::Pause => self.paused = true,
            LoopCmd::Resume => self.paused = false,
            LoopCmd::Step => {
                self.paused = false;
                self.stepping = true;
            }
            LoopCmd::Skip | LoopCmd::Abort => {
                self.paused = false;
                self.pending = Some(cmd);
            }
        }
    }

    /// The next thing to act on: a tapped message (roadmap completions
    /// counted on the way), a command, a gate outcome, a tick, or the
    /// cockpit gone.
    async fn wake(
        &mut self,
        gates: Option<&mut UnboundedReceiver<GateOutcome>>,
        ticks: Option<&mut Interval>,
    ) -> Wake {
        let (tap, commands) = (&mut self.tap_rx, &mut self.commands);
        let wake = tokio::select! {
            msg = tap.recv() => msg.map_or(Wake::Closed, Wake::Tap),
            cmd = commands.next() => cmd.map_or(Wake::Closed, Wake::Cmd),
            outcome = recv_gate(gates) => outcome.map_or(Wake::GatesGone, Wake::Gate),
            () = tick(ticks) => Wake::Tick,
        };
        match wake {
            Wake::Cmd(Cmd::Prompt(text)) if !self.session_active => {
                let rejection = Msg::Agent(AgentEvent::PromptRejected {
                    text,
                    error: "No agent is ready for a message right now. Your draft is kept."
                        .to_owned(),
                });
                self.inbox.send(rejection.clone());
                Wake::Tap(rejection)
            }
            Wake::Cmd(Cmd::RoadmapPrompt(text)) => {
                if self.roadmap_request.is_none() {
                    self.roadmap_request = Some(text);
                    self.inbox.send(Msg::RoadmapRequestQueued);
                } else {
                    self.inbox.send(Msg::Agent(AgentEvent::PromptRejected {
                        text,
                        error: "A roadmap request is already waiting. Cancel it before sending another.".to_owned(),
                    }));
                }
                Wake::Tap(Msg::RoadmapRequestQueued)
            }
            Wake::Cmd(Cmd::CancelRoadmapPrompt) => {
                if self.roadmap_request.take().is_some() {
                    self.inbox.send(Msg::RoadmapRequestCancelled);
                }
                Wake::Tap(Msg::RoadmapRequestCancelled)
            }
            Wake::Tap(msg) => {
                self.observe(&msg);
                Wake::Tap(msg)
            }
            // A ledger verb — the boundary's own save, or a
            // `/checkpoint-now`, `/restore` or `/fork` typed on the
            // loop's cockpit — is the driver's to serve when it carries
            // the port: off this task, and never forwarded to a session
            // that holds no port to drop it on.
            Wake::Cmd(Cmd::Checkpoint(cmd)) if self.checkpoints.is_some() => {
                self.ledger(cmd);
                Wake::Ledger
            }
            other => other,
        }
    }

    /// A roadmap completion event names a chunk this iteration
    /// completed; the agent's own words are kept for the judge.
    fn observe(&mut self, msg: &Msg) {
        match msg {
            Msg::Engine(EngineEvent::Event {
                event: NativeEvent::Roadmap(RoadmapFrame::ChunkCompleted { chunk }),
                ..
            }) => self.completed.push(chunk.id.clone()),
            Msg::Agent(AgentEvent::Text(text)) => self.transcript.push_str(text),
            // ACP's `UsageUpdate.used` is the context's level; the
            // highest level of the turn is what the turn put in context.
            Msg::Agent(AgentEvent::Usage {
                used,
                size,
                cost_cents,
                ..
            }) => {
                tracing::info!(target: "magistr::loop", used, size, "usage");
                self.tokens_seen = Some(self.tokens_seen.map_or(*used, |seen| seen.max(*used)));
                if let Some(cents) = cost_cents {
                    self.iter_loop
                        .observe_budget(self.iteration_cost_base + *cents as f64 / 100.0);
                    if self.persist
                        && let Err(error) = self.iter_loop.persist()
                    {
                        self.iter_loop
                            .stop(magistr_engine::workflow::StopReason::Failed);
                        self.note(format!("could not checkpoint observed spending: {error}"));
                    }
                }
            }
            Msg::Agent(AgentEvent::TurnEnded(end)) => {
                tracing::info!(target: "magistr::loop", ?end, "turn ended");
            }
            _ => {}
        }
    }

    /// Record onto the running iteration's metrics; nothing before the
    /// first iteration starts.
    fn measure(&mut self, record: impl FnOnce(&mut IterationMetrics)) {
        if let Some(metrics) = self.metrics.last_mut() {
            record(metrics);
        }
    }

    fn post(&self, event: LoopEvent) {
        // Every report but the clock goes on the log too, timestamped,
        // for a run measured from outside the cockpit.
        if !matches!(event, LoopEvent::Elapsed(_)) {
            tracing::info!(target: "magistr::loop", ?event, "loop");
        }
        self.inbox.send(Msg::Loop(event));
    }

    fn note(&self, text: impl Into<String>) {
        self.post(LoopEvent::Note(text.into()));
    }
}

/// The provider runtime's own handle for the dispatch record: the process
/// group the transport spawned, pinned to that process by its start
/// identity, read while it lives; nothing for an in-process fake, and
/// nothing for a runtime already gone by the time it is asked (a record
/// that claims no launch can be judged only once something says what it
/// launched — the loop settles such a dispatch itself before it returns).
fn runtime_handle(transport: &magistr_acp::AcpTransport) -> Option<String> {
    u32::try_from(transport.child_pgid())
        .ok()
        .and_then(RuntimeHandle::observe)
        .map(|handle| handle.to_string())
}

fn ticker() -> Interval {
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // The first tick is immediate; the clock starts at zero anyway.
    interval.reset();
    interval
}

/// What awaiting one gate ended in.
enum GateStep {
    /// The gate returned its run.
    Ran(GateOutcome),
    /// The job is gone without reporting.
    Gone,
    /// The cockpit closed mid-gate.
    Closed,
}

/// The next gate outcome, or never when no gates run.
async fn recv_gate(rx: Option<&mut UnboundedReceiver<GateOutcome>>) -> Option<GateOutcome> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

/// A pending extension request's answer; none pending never resolves.
/// The caller clears the slot after a resolution — a spent receiver
/// must not be polled again.
async fn extension_answer(
    rx: &mut Option<oneshot::Receiver<magistr_acp::Result<serde_json::Value>>>,
) -> Result<magistr_acp::Result<serde_json::Value>, oneshot::error::RecvError> {
    match rx.as_mut() {
        Some(rx) => rx.await,
        None => std::future::pending().await,
    }
}

/// The next tapped transport event; an exhausted tap never resolves.
async fn drained(tap: &mut Option<&mut UnboundedReceiver<AcpEvent>>) -> Option<AcpEvent> {
    match tap.as_mut() {
        Some(tap) => tap.recv().await,
        None => std::future::pending().await,
    }
}

/// The next tick, or never when the clock is not running.
async fn tick(ticks: Option<&mut Interval>) {
    match ticks {
        Some(ticks) => {
            ticks.tick().await;
        }
        None => std::future::pending().await,
    }
}

#[cfg(test)]
#[path = "driver.tests.rs"]
mod tests;
