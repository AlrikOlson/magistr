use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use magistr_workflow::content::StructuredContent;
use magistr_workflow::roadmap::application::{BlockerDraft, ChunkDraft, ChunkPatch};
use magistr_workflow::roadmap::domain::{BlockerKind, Chunk, ChunkStatus};
use magistr_workflow::ship::application::{
    CheckAdmission, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, ShipReport,
};
use magistr_workflow::ship::domain::action::{Action, ActionType};
use magistr_workflow::ship::domain::artifact::Artifact;
use magistr_workflow::ship::domain::check::Check;
use magistr_workflow::ship::domain::task::{Task, TaskType};
use magistr_workflow::signal::domain::{Signal, SignalKind};
use magistr_workflow::think::domain::{RecordedStep, StepError, ThinkStep};
use serde_json::Value;

use super::check_type_for::check_type_for;
use super::family_services::FamilyServices;
use super::native_reader::NativeReader;
use super::project_authority::ProjectAuthority;

/// The write capability over the four think-and-ship engines, held only by
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
/// The existing native stores remain authoritative. Source-bound receipt
/// consumption and completion freshness are deferred to arch-14.
#[derive(Clone)]
pub struct NativeEngines {
    reader: NativeReader,
    authority: Arc<ProjectAuthority>,
    services: FamilyServices,
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
        Self {
            reader,
            authority,
            services,
        }
    }

    /// The read capability over the same engines, without the lease: what a
    /// re-seed timer or a progress view holds so that dropping the last
    /// writer releases the role's lock regardless of how many observers
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

    pub fn roadmap_start_chunk(&self, id: &str) -> Result<Chunk, String> {
        self.services
            .roadmap
            .start_chunk(id)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_complete_chunk(
        &self,
        id: &str,
        ship_ref: Option<&str>,
    ) -> Result<Chunk, String> {
        self.services
            .roadmap
            .complete_chunk(id, ship_ref)
            .map_err(|error| error.to_string())
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
    ) -> Result<Chunk, String> {
        self.services
            .roadmap
            .add_chunk(ChunkDraft {
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
            .map_err(|error| error.to_string())
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
    ) -> Result<Chunk, String> {
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

    fn update_chunk(&self, id: &str, patch: ChunkPatch) -> Result<Chunk, String> {
        self.services
            .roadmap
            .update_chunk(id, patch)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_clear_blocked_by(&self, id: &str) -> Result<Chunk, String> {
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
    ) -> Result<Chunk, String> {
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

    pub fn roadmap_set_status(&self, id: &str, to: ChunkStatus) -> Result<Chunk, String> {
        self.services
            .roadmap
            .set_status(id, to)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_obsolete_chunk(&self, id: &str, reason: String) -> Result<Chunk, String> {
        self.services
            .roadmap
            .obsolete_chunk(id, reason)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_reprioritize(
        &self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Chunk, String> {
        self.services
            .roadmap
            .propose_reprioritize(id, suggested_priority, reason)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_propose_title(
        &self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<Chunk, String> {
        self.services
            .roadmap
            .propose_title(id, suggested_title, reason, source)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_propose_status(
        &self,
        id: &str,
        suggested_status: ChunkStatus,
        reason: String,
        source: String,
    ) -> Result<Chunk, String> {
        self.services
            .roadmap
            .propose_status(id, suggested_status, reason, source)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_link(&self, id: &str, cross_ref: &str) -> Result<Chunk, String> {
        self.services
            .roadmap
            .link_chunk(id, cross_ref)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_set_group(&self, id: &str, group: Option<String>) -> Result<Chunk, String> {
        self.services
            .roadmap
            .set_group(id, group)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_resolve_title_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        self.services
            .roadmap
            .resolve_title_proposal(id, accept)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_resolve_status_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        self.services
            .roadmap
            .resolve_status_proposal(id, accept)
            .map_err(|error| error.to_string())
    }

    pub fn roadmap_resolve_reprioritize(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        self.services
            .roadmap
            .resolve_reprioritize(id, accept)
            .map_err(|error| error.to_string())
    }

    // ── Think ──────────────────────────────────────────────────────────

    /// Record a native step with its bound project and stored step identity.
    /// The extensible display projection is serialized only at transport edges.
    pub fn think_record_step(&self, step: ThinkStep) -> Result<RecordedStep, StepError> {
        self.services.think.record_step(step)
    }

    /// Pin or unpin a step (the `think_pin_step` verb); answers whether it
    /// was pinned before.
    pub fn think_pin_step(&self, step_number: u32, pinned: bool) -> Result<bool, String> {
        self.services.think.pin_step(step_number, pinned)
    }

    /// The trace-wide metacognitive checkpoint (the
    /// `think_trace_checkpoint` verb's report): open hypotheses, stale
    /// branches, confidence drift, dependencies on revised steps.
    pub fn think_checkpoint(&self) -> Result<Value, String> {
        self.services.think.checkpoint()
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// Define the objective (the `ship_set_objective` verb); answers how
    /// many stale tasks a fresh cycle cleared.
    pub fn ship_set_objective(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<usize, String> {
        self.services
            .ship
            .set_objective(description, acceptance_criteria, constraints, scope)
            .map(|result| result.cleared_stale_tasks)
    }

    /// Define the objective with an explicit verification scope.
    pub fn ship_set_objective_with_verification(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: magistr_workflow::ship::domain::objective::VerificationMode,
    ) -> Result<usize, String> {
        self.services
            .ship
            .set_objective_with_verification(
                description,
                acceptance_criteria,
                constraints,
                scope,
                verification,
            )
            .map(|result| result.cleared_stale_tasks)
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
    pub fn ship_compact_receipts(&self) -> Result<super::ReceiptCompaction, String> {
        self.services.ship.compact_receipts()
    }

    /// Add a task to the plan (the `ship_plan` verb's `add`).
    pub fn ship_add_task(
        &self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
    ) -> Result<(), String> {
        self.services
            .ship
            .add_task(id, title, task_type, estimate, None)
    }

    /// Begin a task (the `ship_start` verb).
    pub fn ship_start_task(&self, task_id: &str) -> Result<Task, String> {
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
    ) -> Result<Action, String> {
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

    /// Compatibility entry point for an immediately available result.
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
    ) -> Result<Check, String> {
        let admission = self.ship_admit_gate(task_id, gate, command, required)?;
        match self.ship_settle_gate(
            admission,
            CheckExecution::Exited { code: exit_code },
            details,
            report,
        )? {
            CheckSettlement::Applied { check, .. } => Ok(check),
            CheckSettlement::Stale { admission, .. } => Err(format!(
                "check belongs to superseded run {} task {}",
                admission.target.run_id, admission.target.task_id,
            )),
        }
    }

    /// Capture the concrete task instance for a sequence of native operations.
    pub fn ship_admit_task(
        &self,
        task_id: Option<&str>,
    ) -> Result<magistr_workflow::ship::application::TaskAdmission, String> {
        self.services.ship.admit_task(task_id)
    }

    /// Capture native project/run/task identity before a host starts a gate.
    pub fn ship_admit_gate(
        &self,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        required: bool,
    ) -> Result<CheckAdmission, String> {
        self.services
            .ship
            .admit_check(CheckRequest {
                task_id: task_id.map(str::to_owned),
                check_type: check_type_for(gate),
                name: gate.to_owned(),
                passed: None,
                details: String::new(),
                required,
                command: Some(command),
                report: None,
            })
            .map_err(|error| error.to_string())
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
    ) -> Result<CheckSettlement, String> {
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
    ) -> Result<Task, String> {
        self.services.ship.complete_task(task_id, artifacts)
    }

    /// Finalize the objective (the `ship_finalize` verb); answers the report.
    pub fn ship_finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<ShipReport, String> {
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
    ) -> Result<Signal, String> {
        self.services
            .signal
            .capture(kind, from, body, None)
            .map_err(|error| error.to_string())
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
    ) -> Result<Signal, String> {
        self.services
            .signal
            .research(id, summary, confidence, sources, think_step)
            .map_err(|error| error.to_string())
    }

    /// Promote a researched or surfaced signal to the roadmap (the
    /// `signal_promote` verb): the backlog chunk `signal-<id>` is created
    /// and the two are cross-linked — `signal:<id>` on the chunk,
    /// `chunk:<id>` on the signal — idempotently. Answers the signal as it
    /// stands after.
    pub fn signal_promote(&self, id: &str) -> Result<Signal, String> {
        self.services
            .signal
            .promote(id)
            .map(|result| result.signal)
            .map_err(|error| error.to_string())
    }

    /// Dismiss a signal (the `signal_ignore` verb; terminal).
    pub fn signal_ignore(&self, id: &str) -> Result<Signal, String> {
        self.services
            .signal
            .ignore(id)
            .map_err(|error| error.to_string())
    }

    /// Suppress a signal from surfacing for `minutes` (the `signal_snooze`
    /// verb); status unchanged.
    pub fn signal_snooze(&self, id: &str, minutes: i64) -> Result<Signal, String> {
        self.services
            .signal
            .snooze(id, Some(minutes))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
#[path = "native_engines.tests.rs"]
mod tests;
