use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use magistr_workflow::content::StructuredContent;
use magistr_workflow::decision::domain::{Note, NoteError, RecordedNote};
use magistr_workflow::plan::application::PlanError;
use magistr_workflow::plan::application::{TaskDraft, TaskPatch, WaitingDraft};
use magistr_workflow::plan::domain::{Task, TaskStatus, WaitingReason};
use magistr_workflow::request::application::{RequestError, RequestPromotion};
use magistr_workflow::request::domain::{Request, RequestKind};
use magistr_workflow::run::application::RunError;
use magistr_workflow::run::application::{
    CheckAdmission, CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, RunReport,
};
use magistr_workflow::run::domain::artifact::Artifact;
use magistr_workflow::run::domain::check::Check;
use magistr_workflow::run::domain::progress::{Progress, ProgressType};
use magistr_workflow::run::domain::step::{Step, StepType};
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
/// step identity before execution, then settle against that identity.
///
/// The native stores remain authoritative. Contextual evidence, step admission
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
    /// Shared planning stays on this authority; selected steps, check_results and source evidence
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
                    "Close some step sessions before opening more.",
                ));
            }
            let services = Arc::new({
                let mut run =
                    super::open_engines::open_run(&config, self.authority.project_id().to_owned());
                if let Some(feed) = self.authority.feed() {
                    run = run.with_broadcaster(
                        magistr_workflow::run::broadcast::Broadcaster::from_engine(
                            feed.clone().with_execution(context.key()),
                        ),
                    );
                }
                self.services.for_execution(run, config.evidence().cloned())
            });
            executions.insert(key, Arc::downgrade(&services));
            services
        };
        let (decision, run, plan, request) = services.engines();
        let reader = NativeReader::from_shared(
            self.authority.project_id().to_owned(),
            config,
            decision,
            run,
            plan,
            request,
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

    // ── Plan ────────────────────────────────────────────────────────

    pub fn plan_start_task(&self, id: &str) -> Result<Observed<Task>, PlanError> {
        self.services.plan.start_task(id)
    }

    pub fn plan_complete_task(
        &self,
        id: &str,
        step_ref: Option<&str>,
    ) -> Result<Observed<Task>, PlanError> {
        self.services.plan.complete_task(id, step_ref)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn plan_add_task(
        &self,
        id: String,
        title: String,
        name: String,
        status: TaskStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        content: Option<StructuredContent>,
        feature: Option<String>,
    ) -> Result<Observed<Task>, PlanError> {
        self.services.plan.add_task(TaskDraft {
            id,
            title,
            name,
            status,
            priority,
            description,
            acceptance,
            deps,
            content,
            feature,
            shared: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn plan_update_task(
        &self,
        id: &str,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<StructuredContent>,
    ) -> Result<Observed<Task>, PlanError> {
        self.update_task(
            id,
            TaskPatch {
                title,
                priority,
                description,
                acceptance,
                deps,
                content,
                ..TaskPatch::default()
            },
        )
    }

    fn update_task(&self, id: &str, patch: TaskPatch) -> Result<Observed<Task>, PlanError> {
        self.services.plan.update_task(id, patch)
    }

    pub fn plan_clear_waiting_on(&self, id: &str) -> Result<Observed<Task>, PlanError> {
        self.update_task(
            id,
            TaskPatch {
                clear_waiting_on: true,
                ..TaskPatch::default()
            },
        )
    }

    pub fn plan_set_waiting_on(
        &self,
        id: &str,
        kind: WaitingReason,
        reason: String,
        evidence: Option<String>,
    ) -> Result<Observed<Task>, PlanError> {
        self.update_task(
            id,
            TaskPatch {
                waiting_on: Some(WaitingDraft {
                    kind,
                    reason,
                    evidence,
                }),
                ..TaskPatch::default()
            },
        )
    }

    pub fn plan_set_status(&self, id: &str, to: TaskStatus) -> Result<Observed<Task>, PlanError> {
        self.services.plan.set_status(id, to)
    }

    pub fn plan_retire_task(&self, id: &str, reason: String) -> Result<Observed<Task>, PlanError> {
        self.services.plan.retire_task(id, reason)
    }

    pub fn plan_reprioritize(
        &self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Observed<Task>, PlanError> {
        self.services
            .plan
            .propose_reprioritize(id, suggested_priority, reason)
    }

    pub fn plan_propose_title(
        &self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<Observed<Task>, PlanError> {
        self.services
            .plan
            .propose_title(id, suggested_title, reason, source)
    }

    pub fn plan_propose_status(
        &self,
        id: &str,
        suggested_status: TaskStatus,
        reason: String,
        source: String,
    ) -> Result<Observed<Task>, PlanError> {
        self.services
            .plan
            .propose_status(id, suggested_status, reason, source)
    }

    pub fn plan_link(&self, id: &str, cross_ref: &str) -> Result<Observed<Task>, PlanError> {
        self.services.plan.link_task(id, cross_ref)
    }

    pub fn plan_set_feature(
        &self,
        id: &str,
        feature: Option<String>,
    ) -> Result<Observed<Task>, PlanError> {
        self.services.plan.set_feature(id, feature)
    }

    pub fn plan_resolve_title_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanError> {
        self.services.plan.resolve_title_proposal(id, accept)
    }

    pub fn plan_resolve_status_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanError> {
        self.services.plan.resolve_status_proposal(id, accept)
    }

    pub fn plan_resolve_reprioritize(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanError> {
        self.services.plan.resolve_reprioritize(id, accept)
    }

    // ── Decision ──────────────────────────────────────────────────────────

    /// Record a native note with its bound project and stored note identity.
    /// The extensible display projection is serialized only at transport edges.
    pub fn decision_record(&self, note: Note) -> Result<RecordedNote, NoteError> {
        self.services.decision.record(note)
    }

    /// Pin or unpin a note (the `decision_pin` verb); answers whether it
    /// was pinned before.
    pub fn decision_pin(
        &self,
        note_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, NoteError> {
        self.services.decision.pin(note_number, pinned)
    }

    /// The log-wide metacognitive review (the `decision_review` verb's
    /// report): open hypotheses, stale threads, confidence drift,
    /// dependencies on revised notes.
    pub fn decision_review(&self) -> Result<Value, String> {
        self.services.decision.checkpoint(None)
    }

    // ── Run ───────────────────────────────────────────────────────────

    /// Define the goal (the `run_open` verb); answers how
    /// many steps a fresh cycle retained from the preceding goal.
    pub fn run_open(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<Observed<usize>, RunError> {
        self.services
            .run
            .set_goal(description, acceptance_criteria, constraints, scope)
            .map(|result| result.map(|value| value.retained_steps))
    }

    /// Define the goal with an explicit verification scope.
    pub fn run_open_with_verification(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: magistr_workflow::run::domain::goal::VerificationMode,
    ) -> Result<Observed<usize>, RunError> {
        self.services
            .run
            .set_goal_with_verification(
                description,
                acceptance_criteria,
                constraints,
                scope,
                verification,
            )
            .map(|result| result.map(|value| value.retained_steps))
    }

    /// Validate existing check_results against current source and acceptance without
    /// running commands or requiring the verification step to remain active.
    pub fn run_verify_completion(
        &self,
        scope: &str,
        step_id: Option<&str>,
    ) -> Result<
        magistr_workflow::run::application::CompletionEvidence,
        magistr_workflow::run::application::CompletionRefusal,
    > {
        self.services.run.verify_completion(scope, step_id)
    }

    /// Explicitly prune older settled check check_results through the shared use case.
    pub fn run_compact_checks(&self) -> Result<super::CheckResultCompaction, RunError> {
        self.services.run.compact_check_results()
    }

    /// Add a step to the plan (the `arrange_steps` verb's `add`).
    pub fn add_step(
        &self,
        id: String,
        title: String,
        step_type: StepType,
        estimate: Option<String>,
    ) -> Result<Observed<()>, RunError> {
        self.services
            .run
            .add_step(id, title, step_type, estimate, None)
    }

    /// Begin a step (the `start_step` verb).
    pub fn start_step(&self, step_id: &str) -> Result<Observed<Step>, RunError> {
        self.services.run.start_step(step_id)
    }

    /// Record progress on a step (the `record_progress` verb; the active
    /// step when `step_id` is `None`).
    pub fn record_progress(
        &self,
        step_id: Option<&str>,
        progress_type: ProgressType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        note: Option<u32>,
    ) -> Result<Observed<Progress>, RunError> {
        self.services.run.record_progress(
            step_id,
            progress_type,
            description,
            files_touched,
            tools_used,
            String::new(),
            note,
        )
    }

    /// Record an immediately available result against its captured owner.
    /// Async runners must capture `run_admit_gate` before execution and pass
    /// that identity to `run_settle_gate` afterward. They must not admit an
    /// already-running command against whichever step is current on return.
    /// Reports are additive; the process exit code alone determines `passed`.
    #[allow(clippy::too_many_arguments)]
    pub fn run_record_gate(
        &self,
        step_id: Option<&str>,
        gate: &str,
        command: String,
        exit_code: i32,
        details: String,
        required: bool,
        report: Option<(&str, &str)>,
    ) -> Result<Observed<Check>, RunError> {
        let admission = self.run_admit_gate(step_id, gate, command, required)?.value;
        match self.run_settle_gate(
            admission,
            CheckExecution::Exited { code: exit_code },
            details,
            report,
        )? {
            CheckSettlement::Applied { check, .. } => Ok(check),
            CheckSettlement::Stale { .. } => Err(RunError::Refused {
                kind: "stale_check",
                message: "This result belongs to earlier work. Read the saved check check_result before continuing.".into(),
            }),
        }
    }

    /// Capture the concrete step instance for a sequence of native operations.
    pub fn run_admit_step(
        &self,
        step_id: Option<&str>,
    ) -> Result<magistr_workflow::run::application::StepAdmission, RunError> {
        self.services.run.admit_step(step_id)
    }

    /// Capture native project/run/step identity before a host starts a gate.
    pub fn run_admit_gate(
        &self,
        step_id: Option<&str>,
        gate: &str,
        command: String,
        required: bool,
    ) -> Result<Observed<CheckAdmission>, RunError> {
        self.services.run.admit_check(CheckRequest {
            options: Default::default(),
            step_id: step_id.map(str::to_owned),
            check_type: check_type_for(gate),
            name: gate.to_owned(),
            passed: None,
            details: String::new(),
            required,
            command: Some(command),
            report: None,
        })
    }

    /// Find current evidence for this command under its new step admission.
    pub fn run_reuse_gate(
        &self,
        admission: &CheckAdmission,
    ) -> Result<Option<CheckOutcome>, RunError> {
        self.services.run.reuse_check(admission)
    }

    /// Admit a caller's bounded check through the canonical policy owner.
    pub fn run_admit_check(
        &self,
        request: CheckRequest,
    ) -> Result<Observed<CheckAdmission>, RunError> {
        self.services.run.admit_check(request)
    }

    /// Settle only against the captured native identity. A stale result retains
    /// its admission and execution data for recovery, never another step; the
    /// check_result is settled under its original identity either way. Only an
    /// exit decides `passed`: a timeout, a cancel or a launch failure is
    /// recorded as what it was.
    pub fn run_settle_gate(
        &self,
        admission: CheckAdmission,
        execution: CheckExecution,
        details: String,
        report: Option<(&str, &str)>,
    ) -> Result<CheckSettlement, RunError> {
        if matches!(execution, CheckExecution::Reused { .. }) {
            let outcome = self
                .services
                .run
                .reuse_check(&admission)?
                .filter(|outcome| outcome.execution == execution)
                .ok_or_else(|| {
                    RunError::from("The earlier result no longer matches; run the check again.")
                })?;
            return self.services.run.settle_check(admission, outcome);
        }
        let verified = execution.verified();
        let (report, results) = match report.filter(|_| verified) {
            Some((format, path)) => {
                let (record, results) = magistr_workflow::run::report::read_report(format, path);
                (Some(record), results)
            }
            None => (None, None),
        };
        let outcome = CheckOutcome {
            passed: execution.passed(),
            details,
            verified,
            command: admission.request.command.clone(),
            exit_code: execution.exit_code(),
            execution,
            report,
            results,
        };
        self.services.run.settle_check(admission, outcome)
    }

    /// Close a step (the `finish_step` verb).
    pub fn finish_step(
        &self,
        step_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Observed<Step>, RunError> {
        self.services.run.complete_step(step_id, artifacts)
    }

    /// Finalize the goal (the `run_finish` verb); answers the report.
    pub fn run_finish(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Observed<RunReport>, RunError> {
        self.services.run.finalize(artifacts, summary)
    }

    // ── Request ─────────────────────────────────────────────────────────

    /// Capture a stakeholder request (the `request_capture` verb). An empty
    /// body is refused, as the verb refuses it.
    pub fn request_capture(
        &self,
        kind: RequestKind,
        from: String,
        body: String,
    ) -> Result<Observed<Request>, RequestError> {
        self.services.request.capture(kind, from, body, None)
    }

    /// Record research on a request (the `request_research` verb): a summary,
    /// a confidence clamped to `[0, 1]`, sources, and the reasoning step
    /// that motivated it. Moves a new request to `Researched`.
    pub fn request_research(
        &self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        note: Option<u32>,
    ) -> Result<Observed<Request>, RequestError> {
        self.services
            .request
            .research(id, summary, confidence, sources, note)
    }

    /// Promote a researched or raised request to the plan (the
    /// `request_promote` verb): the idea task `request-<id>` is created
    /// and the two are cross-linked — `request:<id>` on the task,
    /// `task:<id>` on the request — idempotently. Answers the request as it
    /// stands after.
    pub fn request_promote(&self, id: &str) -> Result<RequestPromotion, RequestError> {
        self.services.request.promote(id)
    }

    /// Dismiss a request (the `request_dismiss` verb; terminal).
    pub fn request_dismiss(&self, id: &str) -> Result<Observed<Request>, RequestError> {
        self.services.request.dismiss(id)
    }

    /// Suppress a request from raising for `minutes` (the `request_snooze`
    /// verb); status unchanged.
    pub fn request_snooze(
        &self,
        id: &str,
        minutes: i64,
    ) -> Result<Observed<Request>, RequestError> {
        self.services.request.snooze(id, Some(minutes))
    }
}

#[cfg(test)]
#[path = "native_engines.tests.rs"]
mod tests;
