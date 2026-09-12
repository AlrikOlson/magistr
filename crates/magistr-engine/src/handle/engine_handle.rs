use std::sync::{Arc, Mutex};

use chrono::Utc;

use tokio::sync::broadcast;

use crate::cost::{BudgetConfig, CostGovernor};
use crate::error::Result;
use crate::native::{NativeReader, NativeRoadmapSnapshot};
use crate::project::ProjectContext;
use crate::roadmap::RoadmapEvent;
use crate::roadmap::{RoadmapManager, RoadmapProgress};
use crate::session::{
    EngineSession, RestoredSession, SessionCommand, SessionEvent, SessionSnapshot, SessionStatus,
};
use crate::workflow::lifecycle_event::LifecycleEvent;
use crate::workflow::{
    HookResolver, RunMode, StopReason, WorkflowCommand, WorkflowConfig, WorkflowEngine,
    WorkflowPhase,
};

use super::engine_event::EngineEvent;
use super::workflow_event::WorkflowEvent;

/// Internal mutable state behind the `Arc<Mutex<...>>`.
#[derive(Debug)]
struct EngineState {
    session: EngineSession,
    workflow: WorkflowEngine,
    cost_governor: CostGovernor,
    roadmap: RoadmapManager,
    project_context: ProjectContext,
}

/// Default broadcast channel capacity.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// The primary entry point for the magistr engine.
///
/// Composes all domain services behind a typed API. Thread-safe:
/// `Clone + Send + Sync`. Cloning produces a handle to the same
/// underlying state.
///
/// # Event subscription
///
/// Call `subscribe()` to get a `broadcast::Receiver<EngineEvent>`.
/// Events are broadcast to all subscribers. If a subscriber falls
/// behind, it receives a `RecvError::Lagged` with the count of
/// missed events.
#[derive(Debug, Clone)]
pub struct EngineHandle {
    state: Arc<Mutex<EngineState>>,
    event_tx: broadcast::Sender<EngineEvent>,
    /// The in-process think-and-ship reader, when the project's
    /// `roadmap_backend` is `think-and-ship`. Outside the state mutex: the
    /// engines carry their own locks and must never be held under ours.
    native: Option<NativeReader>,
}

impl EngineHandle {
    /// Create a new engine handle.
    ///
    /// # Arguments
    /// - `session_id` — unique identifier for this session
    /// - `workflow_config` — workflow orchestration configuration
    /// - `budget_config` — budget/spending configuration
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        workflow_config: WorkflowConfig,
        budget_config: BudgetConfig,
    ) -> Self {
        Self::with_run_mode(
            session_id,
            workflow_config,
            budget_config,
            RunMode::ChunkLoop,
        )
    }

    /// Construct a handle with an explicit run mode, separate from its host.
    #[must_use]
    pub fn with_run_mode(
        session_id: impl Into<String>,
        workflow_config: WorkflowConfig,
        budget_config: BudgetConfig,
        mode: RunMode,
    ) -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let mut workflow = WorkflowEngine::with_run_mode(workflow_config, mode);
        workflow.observe_budget(0.0, budget_config.limit);
        let state = EngineState {
            session: EngineSession::new(session_id),
            workflow,
            cost_governor: CostGovernor::new(budget_config),
            roadmap: RoadmapManager::empty(),
            project_context: ProjectContext::empty(),
        };
        Self {
            state: Arc::new(Mutex::new(state)),
            event_tx,
            native: None,
        }
    }

    /// Attach the native think-and-ship reader and seed roadmap progress
    /// from its chunks. A reader, not a writer: the handle only derives its
    /// cursor and progress from the roadmap.
    #[must_use]
    pub fn with_native(mut self, native: NativeReader) -> Self {
        self.native = Some(native);
        self.refresh_native_roadmap();
        self
    }

    /// The native reader, if this handle runs on the think-and-ship backend.
    #[must_use]
    pub fn native(&self) -> Option<&NativeReader> {
        self.native.as_ref()
    }

    /// Subscribe to the engine event stream.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_tx.subscribe()
    }

    // ── Session commands ────────────────────────────────────────────────

    /// Validate and record a session command.
    ///
    /// Does not execute the command (that's the infrastructure adapter's
    /// job). Validates that the command is legal in the current state.
    pub fn validate_command(&self, command: &SessionCommand) -> Result<()> {
        let state = self.lock();
        state.session.validate_command(command)
    }

    /// Update the session status.
    pub fn set_session_status(&self, status: SessionStatus) -> Result<()> {
        let mut state = self.lock();
        state.session.set_status(status)?;
        match status {
            SessionStatus::Cancelled => state.workflow.stop(StopReason::Cancelled),
            SessionStatus::Failed => state.workflow.stop(StopReason::Failed),
            _ => {}
        }
        if status.is_terminal() {
            self.emit(EngineEvent::Session(SessionEvent::SessionEnded {
                status,
                summary: None,
            }));
        }
        Ok(())
    }

    /// Get current session status.
    #[must_use]
    pub fn session_status(&self) -> SessionStatus {
        self.lock().session.status()
    }

    /// Get the session ID.
    #[must_use]
    pub fn session_id(&self) -> String {
        self.lock().session.session_id().to_owned()
    }

    // ── Persistence and migration ───────────────────────────────────────

    /// Write this session down so another process can pick it up.
    ///
    /// Declares no residencies: the engine knows nothing about ACP child
    /// processes, terminals or the workbench lock. The adapter that does
    /// adds them with [`SessionSnapshot::resident_on`] before handing the
    /// document anywhere.
    ///
    /// The roadmap cursor is *derived* rather than tracked: it is the
    /// first chunk the native roadmap reports in progress, or the next
    /// ready one when nothing is. There is no second copy of "what this
    /// session is working on" to fall out of step with the roadmap
    /// itself.
    #[must_use]
    pub fn capture(&self) -> SessionSnapshot {
        // The native engines carry their own locks and must never be
        // held under ours, so this read happens before the state lock.
        let cursor = self.native.as_ref().and_then(|native| {
            let snapshot = native.roadmap_snapshot();
            snapshot
                .in_progress
                .first()
                .cloned()
                .or(snapshot.next_ready)
        });

        let state = self.lock();
        SessionSnapshot::capture(
            &state.session,
            state.workflow.iteration().clone(),
            Utc::now(),
        )
        .with_run(state.workflow.run().clone())
        .with_cost_governor(state.cost_governor.clone())
        .with_roadmap(state.roadmap.progress().clone(), cursor)
    }

    /// Take a snapshot up in this process.
    ///
    /// Refuses — touching nothing — when the document is from a format
    /// this build does not read, or when it carries a residency the
    /// restored session could not honestly survive losing. On success the
    /// session, its phase, its iteration and its roadmap progress are
    /// this handle's, and the returned [`RestoredSession`] names every
    /// machine-bound thing the move left behind.
    ///
    /// The phase is *forced* rather than advanced: a resume is not a
    /// transition, it is the same session continuing, and
    /// [`WorkflowEngine::advance_phase`] rightly refuses to walk
    /// backwards to a phase this fresh engine has not reached.
    ///
    /// [`WorkflowEngine::advance_phase`]: crate::workflow::WorkflowEngine::advance_phase
    pub fn resume(&self, snapshot: &SessionSnapshot) -> Result<RestoredSession> {
        let restored = snapshot.restore()?;

        let mut state = self.lock();
        let fresh = state.session.status() == SessionStatus::Created
            && state.workflow.phase() == WorkflowPhase::Researching
            && state.workflow.run().is_pristine();
        if !fresh {
            if state.session.session_id() != restored.session.session_id() {
                return Err(crate::EngineError::Precondition(
                    "cannot replace a live session with another run's checkpoint".into(),
                ));
            }
            restored
                .run
                .validate_recovery(state.workflow.run())
                .map_err(|message| crate::EngineError::Precondition(message.into()))?;
            if !restored
                .cost_governor
                .preserves_observations(&state.cost_governor)
            {
                return Err(crate::EngineError::Precondition(
                    "checkpoint would rewind known cost or budget observations".into(),
                ));
            }
        }
        state
            .workflow
            .restore_run(restored.iteration.clone(), restored.run.clone())?;
        state.session = restored.session.clone();
        state.workflow.force_phase(restored.session.phase());
        state.cost_governor = restored.cost_governor.clone();
        state
            .workflow
            .update_spec_progress(restored.roadmap.checked_tasks, restored.roadmap.total_tasks);
        let event = state.roadmap.update_progress(restored.roadmap.clone());
        drop(state);

        if let Some(event) = event {
            self.emit(EngineEvent::Roadmap(event));
        }
        Ok(restored)
    }

    // ── Workflow commands ───────────────────────────────────────────────

    /// Advance the workflow phase.
    pub fn set_phase(&self, phase: WorkflowPhase) -> Result<()> {
        let mut state = self.lock();
        let from = state.workflow.phase();
        state.workflow.advance_phase(phase)?;
        state.session.set_phase(phase);
        let hooks = state.workflow.config().lifecycle_hooks.clone();
        drop(state);

        // Emit OnExit for the previous phase, then PhaseTransition, then OnEnter for the new phase.
        self.emit_lifecycle_hooks(&LifecycleEvent::OnExit(from), &hooks);
        self.emit(EngineEvent::Workflow(WorkflowEvent::PhaseTransition {
            from,
            to: phase,
        }));
        self.emit_lifecycle_hooks(&LifecycleEvent::OnEnter(phase), &hooks);
        Ok(())
    }

    /// Get current workflow phase.
    #[must_use]
    pub fn phase(&self) -> WorkflowPhase {
        self.lock().workflow.phase()
    }

    /// Record a committed iteration.
    pub fn record_committed(&self) {
        let mut state = self.lock();
        let iteration = state.workflow.display_iteration();
        state.workflow.record_committed();
        drop(state);

        self.emit(EngineEvent::Workflow(WorkflowEvent::IterationCompleted {
            iteration,
            committed: true,
        }));
    }

    /// Record a failed iteration.
    pub fn record_failed(&self) {
        let mut state = self.lock();
        state.workflow.record_failed();
    }

    /// Advance to the next iteration.
    pub fn advance_iteration(&self) {
        let state = self.lock();
        let hooks = state.workflow.config().lifecycle_hooks.clone();
        // Emit OnIterationEnd for the finishing iteration before advancing.
        drop(state);
        self.emit_lifecycle_hooks(&LifecycleEvent::OnIterationEnd, &hooks);

        let mut state = self.lock();
        state.workflow.advance_iteration();
        let iteration = state.workflow.display_iteration();
        drop(state);

        self.emit(EngineEvent::Workflow(WorkflowEvent::IterationStarted {
            iteration,
        }));
        self.emit_lifecycle_hooks(&LifecycleEvent::OnIterationStart, &hooks);
    }

    /// Whether the workflow loop should stop.
    #[must_use]
    pub fn should_stop(&self) -> bool {
        self.lock().workflow.should_stop()
    }

    /// Whether the workflow loop is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.lock().workflow.is_paused()
    }

    /// Execute a workflow command (validate + apply + emit events).
    pub fn workflow_command(&self, command: WorkflowCommand) -> Result<()> {
        let mut state = self.lock();
        state.workflow.execute_command(&command)?;
        let iteration = state.workflow.display_iteration();
        drop(state);

        // Emit appropriate event for the command.
        let event = match &command {
            WorkflowCommand::StartIteration => WorkflowEvent::IterationStarted { iteration },
            WorkflowCommand::Pause => WorkflowEvent::LoopStopped {
                reason: "paused by user".into(),
            },
            WorkflowCommand::Resume => WorkflowEvent::IterationStarted { iteration },
            WorkflowCommand::Skip => WorkflowEvent::IterationCompleted {
                iteration: iteration - 1,
                committed: false,
            },
            WorkflowCommand::AbortLoop => WorkflowEvent::LoopStopped {
                reason: "aborted by user".into(),
            },
        };
        self.emit(EngineEvent::Workflow(event));
        Ok(())
    }

    // ── Cost commands ───────────────────────────────────────────────────

    /// Record a completed turn with cost data.
    pub fn record_cost(&self, cost_usd: f64, input_tokens: u64, output_tokens: u64) {
        let mut state = self.lock();
        state
            .session
            .record_turn(cost_usd, input_tokens, output_tokens);
        let events = state
            .cost_governor
            .record_turn(cost_usd, input_tokens, output_tokens);
        let total = state.cost_governor.total_cost_usd();
        let limit = state.cost_governor.budget().limit;
        state.workflow.observe_budget(total, limit);
        drop(state);

        for event in events {
            self.emit(EngineEvent::Cost(event));
        }
    }

    /// Current total cost in USD.
    #[must_use]
    pub fn total_cost_usd(&self) -> f64 {
        self.lock().cost_governor.total_cost_usd()
    }

    /// Whether the budget has been exceeded.
    #[must_use]
    pub fn is_budget_exceeded(&self) -> bool {
        self.lock().cost_governor.is_exceeded()
    }

    /// Extend the budget limit.
    pub fn extend_budget(&self, new_limit: f64) {
        let mut state = self.lock();
        if let Some(event) = state.cost_governor.extend_budget(new_limit) {
            let total = state.cost_governor.total_cost_usd();
            let limit = state.cost_governor.budget().limit;
            state.workflow.observe_budget(total, limit);
            drop(state);
            self.emit(EngineEvent::Cost(event));
        }
    }

    // ── Roadmap commands ────────────────────────────────────────────────

    /// Update roadmap progress from an external source.
    pub fn update_roadmap_progress(&self, progress: RoadmapProgress) {
        let mut state = self.lock();
        state
            .workflow
            .update_spec_progress(progress.checked_tasks, progress.total_tasks);
        if let Some(event) = state.roadmap.update_progress(progress) {
            drop(state);
            self.emit(EngineEvent::Roadmap(event));
        }
    }

    /// Current roadmap progress.
    #[must_use]
    pub fn roadmap_progress(&self) -> RoadmapProgress {
        self.lock().roadmap.progress().clone()
    }

    /// Whether the roadmap spec is complete.
    #[must_use]
    pub fn is_spec_complete(&self) -> bool {
        self.lock().roadmap.is_complete()
    }

    /// Re-read the native roadmap: feeds `RoadmapManager` from the chunks
    /// and emits a `NativeSnapshot` event. `None` without native engines.
    pub fn refresh_native_roadmap(&self) -> Option<NativeRoadmapSnapshot> {
        let snapshot = self.native.as_ref()?.roadmap_snapshot();
        self.update_roadmap_progress(snapshot.progress.clone());
        self.emit(EngineEvent::Roadmap(RoadmapEvent::NativeSnapshot {
            snapshot: snapshot.clone(),
        }));
        Some(snapshot)
    }

    // ── Project context ─────────────────────────────────────────────────

    /// Update the project context snapshot.
    pub fn set_project_context(&self, ctx: ProjectContext) {
        self.lock().project_context = ctx;
    }

    /// Get the current project context snapshot.
    #[must_use]
    pub fn project_context(&self) -> ProjectContext {
        self.lock().project_context.clone()
    }

    // ── Internal ────────────────────────────────────────────────────────

    /// Acquire the state lock.
    ///
    /// Panics if the mutex is poisoned (programming error in another thread).
    fn lock(&self) -> std::sync::MutexGuard<'_, EngineState> {
        self.state.lock().expect("engine state mutex poisoned")
    }

    /// Emit an event to all subscribers. Silently drops if no subscribers.
    fn emit(&self, event: EngineEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Resolve and emit lifecycle hook events for the given lifecycle event.
    ///
    /// No-op if no hooks match.
    fn emit_lifecycle_hooks(
        &self,
        event: &LifecycleEvent,
        hooks: &[crate::workflow::LifecycleHook],
    ) {
        let resolved = HookResolver::resolve(event, hooks);
        if !resolved.is_empty() {
            let tools: Vec<String> = resolved
                .iter()
                .map(|h| h.display_name().to_owned())
                .collect();
            self.emit(EngineEvent::Workflow(
                WorkflowEvent::LifecycleHooksTriggered {
                    event: event.clone(),
                    hook_count: resolved.len(),
                    tools,
                    hooks: resolved,
                },
            ));
        }
    }

    /// Report the result of hook execution back through the event system.
    pub fn report_hook_result(
        &self,
        event: LifecycleEvent,
        report: &crate::workflow::HookExecutionReport,
    ) {
        let error = if !report.is_success() {
            // Find the first hard failure (non-Warn)
            report
                .results
                .iter()
                .find_map(|r| r.as_ref().err())
                .map(|e| e.message.clone())
        } else {
            None
        };
        self.emit(EngineEvent::Workflow(WorkflowEvent::LifecycleHookResult {
            event,
            success: report.is_success(),
            warnings: report.warnings(),
            error,
        }));
    }
}
