use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use magistr_workflow::content::StructuredContent;
use magistr_workflow::roadmap::application::RoadmapError;
use magistr_workflow::roadmap::application::{BlockerDraft, ChunkDraft, ChunkPatch};
use magistr_workflow::roadmap::domain::{BlockerKind, Chunk, ChunkStatus};
use magistr_workflow::ship::application::ShipError;
use magistr_workflow::ship::application::{
    CheckAdmission, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, ShipReport,
};
use magistr_workflow::ship::domain::action::{Action, ActionType};
use magistr_workflow::ship::domain::artifact::Artifact;
use magistr_workflow::ship::domain::check::Check;
use magistr_workflow::ship::domain::task::{Task, TaskType};
use magistr_workflow::signal::application::{SignalError, SignalPromotion};
use magistr_workflow::signal::domain::{Signal, SignalKind};
use magistr_workflow::think::domain::{RecordedStep, StepError, ThinkStep};
use magistr_workflow_contracts::Observed;
use serde_json::Value;

use super::check_type_for::check_type_for;
use super::family_services::FamilyServices;
use super::native_reader::NativeReader;
use super::project_authority::ProjectAuthority;

/// The write capability over the four magistr engines, held only by
/// an admitted [`ProjectAuthority`] — [`ProjectAuthority::open`] is the one
/// constructor. Derefs to the [`NativeReader`] it was opened with, so every
/// query is available on it too.
///
/// Typed application use cases own validation, reciprocal family behavior and
/// persistence beneath transports. This admitted capability calls them directly;
/// the MCP and owner adapters serialize only at their external boundary.
///
/// Every clone retains the project lease. Readers share the same engine handles
/// without extending write admission. Long-running checks capture native run and
/// task identity before execution, then settle against that identity.
///
/// The native stores remain authoritative. Contextual evidence, task admission
/// and current-source completion checks are owned by the workflow use cases.
#[derive(Clone)]
pub struct NativeEngines {
    reader: NativeReader,
    authority: Arc<ProjectAuthority>,
    services: Arc<FamilyServices>,
    _working_copy: Option<Arc<super::WorkingCopyLease>>,
    executions:
        Arc<std::sync::Mutex<std::collections::BTreeMap<String, std::sync::Weak<FamilyServices>>>>,
}

impl fmt::Debug for NativeEngines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeEngines")
            .field("project_id", &self.reader.project_id())
            .field("role", &self.authority.role())
            .finish_non_exhaustive()
    }
}

impl Deref for NativeEngines {
    type Target = NativeReader;

    fn deref(&self) -> &NativeReader {
        &self.reader
    }
}

impl NativeEngines {
    pub(crate) fn admitted(
        reader: NativeReader,
        authority: Arc<ProjectAuthority>,
        services: FamilyServices,
    ) -> Self {
        let services = Arc::new(services);
        let key = reader.config().execution_context().key();
        let executions = Arc::new(std::sync::Mutex::new(std::collections::BTreeMap::from([(
            key,
            Arc::downgrade(&services),
        )])));
        Self {
            reader,
            authority,
            services,
            executions,
            _working_copy: None,
        }
    }

    /// Start this owner's event feed and attach its existing execution views.
    /// Hosts can open before entering an async runtime; their endpoint starts the publisher.
    pub async fn publish(&self) -> std::io::Result<()> {
        self.authority
            .publish_feed(&self.authority.config().feed_socket_path())
            .await?;
        let feed = self
            .authority
            .feed()
            .ok_or_else(|| std::io::Error::other("The project feed did not start."))?;
        let executions = self
            .executions
            .lock()
            .map_err(|_| std::io::Error::other("Execution owner lock failed."))?;
        for (key, services) in executions.iter() {
            if let Some(services) = services.upgrade() {
                services.bind_feed(feed, key)?;
            }
        }
        Ok(())
    }

    /// Bind an execution view to a registered working copy of this project.
    /// Shared planning stays on this authority; selected tasks, receipts and source evidence
    /// belong to the context and are reused only by that exact context.
    pub fn for_working_copy(
        &self,
        root: &std::path::Path,
        execution_id: &str,
    ) -> std::io::Result<Self> {
        let location = super::ProjectLocation::resolve(root)?;
        let owner = super::ProjectLocation::resolve(self.authority.config().project_root())?;
        if location.authority_root != owner.authority_root {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "This working copy belongs to another project.",
            ));
        }
        super::ProjectLocation::require_open(&location.working_copy)?;
        let working_copy = Arc::new(super::WorkingCopyLease::active(&location.working_copy)?);
        super::ProjectLocation::require_open(&location.working_copy)?;
        let context = super::ExecutionContext::new(&location.working_copy, execution_id)?;
        let config = self.reader.config().for_context(&context)?;
        let mut executions = self
            .executions
            .lock()
            .map_err(|_| std::io::Error::other("Execution owner lock failed."))?;
        executions.retain(|_, services| services.strong_count() != 0);
        let key = context.key();
        let services = if let Some(services) =
            executions.get(&key).and_then(std::sync::Weak::upgrade)
        {
            services
        } else {
            if executions.len() >= 128 {
                return Err(std::io::Error::other(
                    "Close some task sessions before opening more.",
                ));
            }
            let services = Arc::new({
                let mut ship =
                    super::open_engines::open_ship(&config, self.authority.project_id().to_owned());
                if let Some(feed) = self.authority.feed() {
                    ship = ship.with_broadcaster(
                        magistr_workflow::ship::broadcast::Broadcaster::from_engine(
                            feed.clone().with_execution(context.key()),
                        ),
                    );
                }
                self.services
                    .for_execution(ship, config.evidence().cloned())
            });
            executions.insert(key, Arc::downgrade(&services));
            services
        };
        let (think, ship, roadmap, signal) = services.engines();
        let reader = NativeReader::from_shared(
            self.authority.project_id().to_owned(),
            config,
            think,
            ship,
            roadmap,
            signal,
        );
        Ok(Self {
            reader,
            authority: Arc::clone(&self.authority),
            services,
            executions: Arc::clone(&self.executions),
            _working_copy: Some(working_copy),
        })
    }

    /// The read capability over the same engines, without the lease: what a
    /// re-seed timer or a progress view holds so that dropping the last
    /// writer releases the project's lock regardless of how many observers
    /// remain.
    #[must_use]
    pub fn reader(&self) -> NativeReader {
        self.reader.clone()
    }

    /// The admission this writer holds.
    #[must_use]
    pub fn authority(&self) -> &ProjectAuthority {
        &self.authority
    }

    /// The admission, shared — what an owner endpoint keys itself by and
    /// what a transport keeps alive beside its wire.
    #[must_use]
    pub fn shared_authority(&self) -> Arc<ProjectAuthority> {
        Arc::clone(&self.authority)
    }

    /// The family services every verb runs through — what a transport
    /// serves on the wire.
    pub(crate) fn services(&self) -> &FamilyServices {
        &self.services
    }

    // ── Roadmap ────────────────────────────────────────────────────────

    pub fn roadmap_start_chunk(&self, id: &str) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.start_chunk(id)
    }

    pub fn roadmap_complete_chunk(
        &self,
        id: &str,
        ship_ref: Option<&str>,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.complete_chunk(id, ship_ref)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn roadmap_add_chunk(
        &self,
        id: String,
        title: String,
        name: String,
        status: ChunkStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        content: Option<StructuredContent>,
        group: Option<String>,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.add_chunk(ChunkDraft {
            id,
            title,
            name,
            status,
            priority,
            description,
            acceptance,
            deps,
            content,
            group,
            shared: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn roadmap_update_chunk(
        &self,
        id: &str,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<StructuredContent>,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.update_chunk(
            id,
            ChunkPatch {
                title,
                priority,
                description,
                acceptance,
                deps,
                content,
                ..ChunkPatch::default()
            },
        )
    }

    fn update_chunk(&self, id: &str, patch: ChunkPatch) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.update_chunk(id, patch)
    }

    pub fn roadmap_clear_blocked_by(&self, id: &str) -> Result<Observed<Chunk>, RoadmapError> {
        self.update_chunk(
            id,
            ChunkPatch {
                clear_blocked_by: true,
                ..ChunkPatch::default()
            },
        )
    }

    pub fn roadmap_set_blocked_by(
        &self,
        id: &str,
        kind: BlockerKind,
        reason: String,
        evidence: Option<String>,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.update_chunk(
            id,
            ChunkPatch {
                blocked_by: Some(BlockerDraft {
                    kind,
                    reason,
                    evidence,
                }),
                ..ChunkPatch::default()
            },
        )
    }

    pub fn roadmap_set_status(
        &self,
        id: &str,
        to: ChunkStatus,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.set_status(id, to)
    }

    pub fn roadmap_obsolete_chunk(
        &self,
        id: &str,
        reason: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.obsolete_chunk(id, reason)
    }

    pub fn roadmap_reprioritize(
        &self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services
            .roadmap
            .propose_reprioritize(id, suggested_priority, reason)
    }

    pub fn roadmap_propose_title(
        &self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services
            .roadmap
            .propose_title(id, suggested_title, reason, source)
    }

    pub fn roadmap_propose_status(
        &self,
        id: &str,
        suggested_status: ChunkStatus,
        reason: String,
        source: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services
            .roadmap
            .propose_status(id, suggested_status, reason, source)
    }

    pub fn roadmap_link(&self, id: &str, cross_ref: &str) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.link_chunk(id, cross_ref)
    }

    pub fn roadmap_set_group(
        &self,
        id: &str,
        group: Option<String>,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.set_group(id, group)
    }

    pub fn roadmap_resolve_title_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.resolve_title_proposal(id, accept)
    }

    pub fn roadmap_resolve_status_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.resolve_status_proposal(id, accept)
    }

    pub fn roadmap_resolve_reprioritize(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.services.roadmap.resolve_reprioritize(id, accept)
    }

    // ── Think ──────────────────────────────────────────────────────────

    /// Record a native step with its bound project and stored step identity.
    /// The extensible display projection is serialized only at transport edges.
    pub fn think_record_step(&self, step: ThinkStep) -> Result<RecordedStep, StepError> {
        self.services.think.record_step(step)
    }

    /// Pin or unpin a step (the `think_pin_step` verb); answers whether it
    /// was pinned before.
    pub fn think_pin_step(
        &self,
        step_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, StepError> {
        self.services.think.pin_step(step_number, pinned)
    }

    /// The trace-wide metacognitive checkpoint (the
    /// `think_trace_checkpoint` verb's report): open hypotheses, stale
    /// branches, confidence drift, dependencies on revised steps.
    pub fn think_checkpoint(&self) -> Result<Value, String> {
        self.services.think.checkpoint(None)
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// Define the objective (the `ship_set_objective` verb); answers how
    /// many tasks a fresh cycle retained from the preceding objective.
    pub fn ship_set_objective(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<Observed<usize>, ShipError> {
        self.services
            .ship
            .set_objective(description, acceptance_criteria, constraints, scope)
            .map(|result| result.map(|value| value.retained_tasks))
    }

    /// Define the objective with an explicit verification scope.
    pub fn ship_set_objective_with_verification(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: magistr_workflow::ship::domain::objective::VerificationMode,
    ) -> Result<Observed<usize>, ShipError> {
        self.services
            .ship
            .set_objective_with_verification(
                description,
                acceptance_criteria,
                constraints,
                scope,
                verification,
            )
            .map(|result| result.map(|value| value.retained_tasks))
    }

    /// Validate existing receipts against current source and acceptance without
    /// running commands or requiring the verification task to remain active.
    pub fn ship_verify_completion(
        &self,
        scope: &str,
        task_id: Option<&str>,
    ) -> Result<
        magistr_workflow::ship::application::CompletionEvidence,
        magistr_workflow::ship::application::CompletionRefusal,
    > {
        self.services.ship.verify_completion(scope, task_id)
    }

    /// Explicitly prune older settled check receipts through the shared use case.
    pub fn ship_compact_receipts(&self) -> Result<super::ReceiptCompaction, ShipError> {
        self.services.ship.compact_receipts()
    }

    /// Add a task to the plan (the `ship_plan` verb's `add`).
    pub fn ship_add_task(
        &self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
    ) -> Result<Observed<()>, ShipError> {
        self.services
            .ship
            .add_task(id, title, task_type, estimate, None)
    }

    /// Begin a task (the `ship_start` verb).
    pub fn ship_start_task(&self, task_id: &str) -> Result<Observed<Task>, ShipError> {
        self.services.ship.start_task(task_id)
    }

    /// Log an action within a task (the `ship_record` verb; the active
    /// task when `task_id` is `None`).
    pub fn ship_record_action(
        &self,
        task_id: Option<&str>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<Observed<Action>, ShipError> {
        self.services.ship.record_action(
            task_id,
            action_type,
            description,
            files_touched,
            tools_used,
            String::new(),
            think_step,
        )
    }

    /// Record an immediately available result against its captured owner.
    /// Async runners must capture `ship_admit_gate` before execution and pass
    /// that identity to `ship_settle_gate` afterward. They must not admit an
    /// already-running command against whichever task is current on return.
    /// Reports are additive; the process exit code alone determines `passed`.
    #[allow(clippy::too_many_arguments)]
    pub fn ship_record_gate(
        &self,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        exit_code: i32,
        details: String,
        required: bool,
        report: Option<(&str, &str)>,
    ) -> Result<Observed<Check>, ShipError> {
        let admission = self
            .ship_admit_gate(task_id, gate, command, required)?
            .value;
        match self.ship_settle_gate(
            admission,
            CheckExecution::Exited { code: exit_code },
            details,
            report,
        )? {
            CheckSettlement::Applied { check, .. } => Ok(check),
            CheckSettlement::Stale { .. } => Err(ShipError::Refused {
                kind: "stale_check",
                message: "This result belongs to earlier work. Read the saved check receipt before continuing.".into(),
            }),
        }
    }

    /// Capture the concrete task instance for a sequence of native operations.
    pub fn ship_admit_task(
        &self,
        task_id: Option<&str>,
    ) -> Result<magistr_workflow::ship::application::TaskAdmission, ShipError> {
        self.services.ship.admit_task(task_id)
    }

    /// Capture native project/run/task identity before a host starts a gate.
    pub fn ship_admit_gate(
        &self,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        required: bool,
    ) -> Result<Observed<CheckAdmission>, ShipError> {
        self.services.ship.admit_check(CheckRequest {
            options: Default::default(),
            task_id: task_id.map(str::to_owned),
            check_type: check_type_for(gate),
            name: gate.to_owned(),
            passed: None,
            details: String::new(),
            required,
            command: Some(command),
            report: None,
        })
    }

    /// Find current evidence for this command under its new task admission.
    pub fn ship_reuse_gate(
        &self,
        admission: &CheckAdmission,
    ) -> Result<Option<CheckOutcome>, ShipError> {
        self.services.ship.reuse_check(admission)
    }

    /// Admit a caller's bounded check through the canonical policy owner.
    pub fn ship_admit_check(
        &self,
        request: CheckRequest,
    ) -> Result<Observed<CheckAdmission>, ShipError> {
        self.services.ship.admit_check(request)
    }

    /// Settle only against the captured native identity. A stale result retains
    /// its admission and execution data for recovery, never another task; the
    /// receipt is settled under its original identity either way. Only an
    /// exit decides `passed`: a timeout, a cancel or a launch failure is
    /// recorded as what it was.
    pub fn ship_settle_gate(
        &self,
        admission: CheckAdmission,
        execution: CheckExecution,
        details: String,
        report: Option<(&str, &str)>,
    ) -> Result<CheckSettlement, ShipError> {
        if matches!(execution, CheckExecution::Reused { .. }) {
            let outcome = self
                .services
                .ship
                .reuse_check(&admission)?
                .filter(|outcome| outcome.execution == execution)
                .ok_or_else(|| {
                    ShipError::from("The earlier result no longer matches; run the check again.")
                })?;
            return self.services.ship.settle_check(admission, outcome);
        }
        let (report, results) = match report {
            Some((format, path)) => {
                let (record, results) = magistr_workflow::ship::report::read_report(format, path);
                (Some(record), results)
            }
            None => (None, None),
        };
        let outcome = CheckOutcome {
            passed: execution.passed(),
            details,
            verified: true,
            command: admission.request.command.clone(),
            exit_code: execution.exit_code(),
            execution,
            report,
            results,
        };
        self.services.ship.settle_check(admission, outcome)
    }

    /// Close a task (the `ship_complete` verb).
    pub fn ship_complete_task(
        &self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Observed<Task>, ShipError> {
        self.services.ship.complete_task(task_id, artifacts)
    }

    /// Finalize the objective (the `ship_finalize` verb); answers the report.
    pub fn ship_finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Observed<ShipReport>, ShipError> {
        self.services.ship.finalize(artifacts, summary)
    }

    // ── Signal ─────────────────────────────────────────────────────────

    /// Capture a stakeholder signal (the `signal_capture` verb). An empty
    /// body is refused, as the verb refuses it.
    pub fn signal_capture(
        &self,
        kind: SignalKind,
        from: String,
        body: String,
    ) -> Result<Observed<Signal>, SignalError> {
        self.services.signal.capture(kind, from, body, None)
    }

    /// Record research on a signal (the `signal_research` verb): a summary,
    /// a confidence clamped to `[0, 1]`, sources, and the reasoning step
    /// that motivated it. Moves a new signal to `Researched`.
    pub fn signal_research(
        &self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<Observed<Signal>, SignalError> {
        self.services
            .signal
            .research(id, summary, confidence, sources, think_step)
    }

    /// Promote a researched or surfaced signal to the roadmap (the
    /// `signal_promote` verb): the backlog chunk `signal-<id>` is created
    /// and the two are cross-linked — `signal:<id>` on the chunk,
    /// `chunk:<id>` on the signal — idempotently. Answers the signal as it
    /// stands after.
    pub fn signal_promote(&self, id: &str) -> Result<SignalPromotion, SignalError> {
        self.services.signal.promote(id)
    }

    /// Dismiss a signal (the `signal_ignore` verb; terminal).
    pub fn signal_ignore(&self, id: &str) -> Result<Observed<Signal>, SignalError> {
        self.services.signal.ignore(id)
    }

    /// Suppress a signal from surfacing for `minutes` (the `signal_snooze`
    /// verb); status unchanged.
    pub fn signal_snooze(&self, id: &str, minutes: i64) -> Result<Observed<Signal>, SignalError> {
        self.services.signal.snooze(id, Some(minutes))
    }
}

#[cfg(test)]
#[path = "native_engines.tests.rs"]
mod tests;
