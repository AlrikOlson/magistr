use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_workflow::decision::domain::{Log, Note};
use magistr_workflow::decision::engine::ReasoningServer;
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::domain::{
    FocusState, Plan, Task, WorkstreamCatalog, WorkstreamFrontier, WorkstreamResolution,
    next_ready, workstream_catalog, workstream_frontier,
};
use magistr_workflow::request::RequestEngine;
use magistr_workflow::request::domain::{Request, Requests};
use magistr_workflow::run::domain::check_result::CheckResult;
use magistr_workflow::run::domain::goal::Goal;
use magistr_workflow::run::domain::step::Step;
use magistr_workflow::run::engine::RunEngine;

use super::native_config::NativeConfig;
use super::open_engines::open_engines;
use super::snapshot::NativePlanSnapshot;
use crate::plan::PlanProgress;
use magistr_workflow_contracts::{
    DecisionData, NativeSnapshot, Observed, PlanData, ReadError, RunData,
};

/// Validated queries over the four workflow owners.
/// Every observation reads the owner's actual selected source. Clones share
/// the most recent successful family values for explicitly stale snapshots;
/// direct queries never return those retained values after a failed read.
#[derive(Clone)]
pub struct NativeReader {
    project_id: String,
    /// Captured configuration used to open these owners; each observation
    /// obtains its source from the owner that performed the read.
    config: NativeConfig,
    plan: Arc<Mutex<PlanEngine>>,
    decision: Arc<Mutex<ReasoningServer>>,
    run: Arc<Mutex<RunEngine>>,
    request: Arc<Mutex<RequestEngine>>,
    observations: Arc<Mutex<NativeSnapshot>>,
}

fn require_current_track(catalog: &WorkstreamCatalog, track: &str) -> Result<(), String> {
    let refusal = match catalog.focus_state(Some(track)) {
        FocusState::Current { .. } => return Ok(()),
        FocusState::Closed { feature } => WorkstreamResolution::Closed {
            feature,
            choices: catalog.current_names(),
        },
        FocusState::Missing { .. } | FocusState::Unset => WorkstreamResolution::Unknown {
            choices: catalog.current_names(),
        },
    };
    Err(refusal.explain(track))
}

impl fmt::Debug for NativeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeReader")
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl NativeReader {
    /// Observe the project at `config.project_root`, resolving its identity
    /// the way the external server would. No lease is taken.
    #[must_use]
    pub fn open(config: &NativeConfig) -> Self {
        Self::open_for_project(config, config.resolve_project_id())
    }

    /// Observe the store under an explicit project id. No lease is taken.
    #[must_use]
    pub fn open_for_project(config: &NativeConfig, project_id: String) -> Self {
        // Event publication belongs to the admitted project writer. An
        // observer opens no publisher and never acquires a write lease.
        let (decision, run, plan, request) = open_engines(config, project_id.clone(), None);
        Self::from_engines(project_id, config.clone(), decision, run, plan, request)
    }

    pub(crate) fn from_engines(
        project_id: String,
        config: NativeConfig,
        decision: ReasoningServer,
        run: RunEngine,
        plan: PlanEngine,
        request: RequestEngine,
    ) -> Self {
        Self::from_shared(
            project_id,
            config,
            Arc::new(Mutex::new(decision)),
            Arc::new(Mutex::new(run)),
            Arc::new(Mutex::new(plan)),
            Arc::new(Mutex::new(request)),
        )
    }

    /// A reader over engines something else already holds — the family
    /// services an admitted writer acts through, so its queries and its
    /// handlers look at the same engines.
    pub(crate) fn from_shared(
        project_id: String,
        config: NativeConfig,
        decision: Arc<Mutex<ReasoningServer>>,
        run: Arc<Mutex<RunEngine>>,
        plan: Arc<Mutex<PlanEngine>>,
        request: Arc<Mutex<RequestEngine>>,
    ) -> Self {
        Self {
            project_id,
            config,
            plan,
            decision,
            run,
            request,
            observations: Arc::new(Mutex::new(NativeSnapshot::default())),
        }
    }

    /// The project every engine is keyed by.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// The store configuration the engines were opened on.
    #[must_use]
    pub fn config(&self) -> &NativeConfig {
        &self.config
    }

    /// Observe and update the retained family state while holding its owner
    /// lock. A racing clone therefore cannot replace a newer observation with
    /// an older result after the owner lock has been released.
    fn observe_plan(&self) -> Result<Observed<Plan>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.plan);
        let result = engine.observe();
        let view = result.as_ref().map(|observed| Observed {
            value: PlanData {
                tasks: observed.value.tasks.clone(),
                progress: super::progress::progress_from_tasks(&observed.value.tasks),
            },
            source: observed.source,
        });
        lock(&self.observations)
            .plan
            .record(view.map_err(Clone::clone));
        result
    }

    fn observe_run(&self) -> Result<Observed<RunData>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.run);
        let result = engine.observe();
        lock(&self.observations).run.record(result.clone());
        result
    }

    fn observe_request(&self) -> Result<Observed<Requests>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.request);
        let result = engine.observe();
        let view = result.as_ref().map(|observed| Observed {
            value: observed.value.requests.clone(),
            source: observed.source,
        });
        lock(&self.observations)
            .requests
            .record(view.map_err(Clone::clone));
        result
    }

    fn observe_decision(&self) -> Result<Observed<DecisionData>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.decision);
        let result = engine.observe_logs();
        lock(&self.observations).decisions.record(result.clone());
        result
    }

    /// Read every family independently. Failures are part of the returned
    /// state, retaining only successfully observed records from this reader.
    #[must_use]
    pub fn snapshot(&self) -> NativeSnapshot {
        let _ = self.observe_plan();
        let _ = self.observe_run();
        let _ = self.observe_request();
        let _ = self.observe_decision();
        lock(&self.observations).clone()
    }

    /// Complete logs from the same validated owner used by snapshots and MCP.
    pub fn decision_logs(&self) -> Result<DecisionData, String> {
        Ok(self
            .observe_decision()
            .map_err(|error| error.to_string())?
            .value)
    }

    /// None selects the default; a named log is resolved by the owner.
    pub fn decision_log(&self, log: Option<&str>) -> Result<Option<Log>, String> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id).map_err(|e| e.to_string())?;
        lock(&self.decision)
            .observe_log(log)
            .map(|read| read.value)
            .map_err(|error| error.to_string())
    }

    /// Stable project-wide lookup across every complete log.
    pub fn note(&self, note_number: u32) -> Result<Option<Note>, String> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id).map_err(|e| e.to_string())?;
        lock(&self.decision)
            .observe_note(note_number, false)
            .map(|read| read.value)
            .map_err(|error| error.to_string())
    }

    // ── Plan ────────────────────────────────────────────────────────

    /// Counts, next ready task, current workstreams, history and progress.
    pub fn plan_snapshot(&self) -> Result<NativePlanSnapshot, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        Ok(NativePlanSnapshot::from_tasks(
            &self.project_id,
            &observed.value.tasks,
        ))
    }

    /// The progress derived from one validated plan observation.
    pub fn plan_progress(&self) -> Result<PlanProgress, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        Ok(super::progress::progress_from_tasks(&observed.value.tasks))
    }

    /// The next ready task, including unassigned work.
    pub fn plan_next(&self) -> Result<Option<Task>, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        let tasks = &observed.value.tasks;
        Ok(next_ready(
            tasks,
            |task| {
                task.readiness(|id| {
                    tasks
                        .iter()
                        .find(|candidate| candidate.id == id)
                        .map(|candidate| candidate.status)
                })
                .is_ready()
            },
            |task| task.priority,
        )
        .cloned())
    }

    /// The next ready task in an exactly named current workstream.
    /// Closed or missing saved names never select similarly named work.
    pub fn plan_next_in_track(&self, track: &str) -> Result<Option<Task>, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        let tasks = &observed.value.tasks;
        require_current_track(&workstream_catalog(tasks), track)?;
        Ok(next_ready(
            tasks,
            |task| {
                task.feature.as_deref() == Some(track)
                    && task
                        .readiness(|id| {
                            tasks
                                .iter()
                                .find(|candidate| candidate.id == id)
                                .map(|candidate| candidate.status)
                        })
                        .is_ready()
            },
            |task| task.priority,
        )
        .cloned())
    }

    /// Current workstreams, authored history and unassigned work from one read.
    pub fn plan_workstreams(&self) -> Result<WorkstreamCatalog, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        Ok(workstream_catalog(&observed.value.tasks))
    }

    /// Current work in an exact workstream; None selects unassigned work.
    pub fn plan_frontier(&self, feature: Option<&str>) -> Result<WorkstreamFrontier, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        let tasks = &observed.value.tasks;
        if let Some(feature) = feature {
            require_current_track(&workstream_catalog(tasks), feature)?;
        }
        Ok(workstream_frontier(tasks, feature))
    }

    /// Resolve an explicit current name or unambiguous fragment without writing.
    /// An exact authored closed name is a refusal before fragment matching.
    pub fn plan_resolve_track(&self, query: &str) -> Result<String, String> {
        match self.plan_workstreams()?.resolve_current(query) {
            WorkstreamResolution::Current { feature } => Ok(feature),
            refusal => Err(refusal.explain(query)),
        }
    }

    /// One current task by id.
    pub fn plan_task(&self, id: &str) -> Result<Option<Task>, String> {
        Ok(self.plan_tasks()?.into_iter().find(|task| task.id == id))
    }

    /// Every task in stored order, after a successful observation.
    pub fn plan_tasks(&self) -> Result<Vec<Task>, String> {
        Ok(self
            .observe_plan()
            .map_err(|error| error.to_string())?
            .value
            .tasks)
    }

    /// The owner's bounded status projection of validated records.
    pub fn plan_status(&self) -> Result<serde_json::Value, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        Ok(PlanEngine::status_of(&observed.value))
    }

    /// The owner's markdown or JSON projection of validated records.
    pub fn plan_export(&self, format: &str) -> Result<String, String> {
        let observed = self.observe_plan().map_err(|error| error.to_string())?;
        Ok(PlanEngine::export_of(&observed.value, format))
    }

    // ── Run ───────────────────────────────────────────────────────────

    /// The goal and complete plan from one successful owner read.
    pub fn run_snapshot(&self) -> Result<RunData, String> {
        Ok(self.observe_run().map_err(|error| error.to_string())?.value)
    }

    /// Resolve a selected or retained execution by its stable run identity.
    /// Missing work is distinct from a failed owner read.
    pub fn run_run(&self, run_id: &str) -> Result<Option<magistr_workflow_contracts::Run>, String> {
        Ok(self.run_snapshot()?.run(run_id).cloned())
    }

    pub fn run_goal(&self) -> Result<Option<Goal>, String> {
        Ok(self.run_snapshot()?.current.map(|run| run.goal))
    }

    pub fn run_active_step_id(&self) -> Result<Option<String>, String> {
        Ok(self
            .run_snapshot()?
            .active_step()
            .map(|step| step.id.clone()))
    }

    /// Read the exact durable check_result through its own ledger. Its authority
    /// does not depend on whether the separate execution summary is readable.
    pub fn run_check_result(&self, id: &str) -> Result<Option<CheckResult>, String> {
        let ledger = lock(&self.run).check_results().clone();
        ledger.get(id).map_err(|error| error.to_string())
    }

    pub fn run_steps(&self) -> Result<Vec<Step>, String> {
        Ok(self
            .run_snapshot()?
            .current
            .map_or_else(Vec::new, |run| run.steps))
    }

    // ── Request ─────────────────────────────────────────────────────────

    /// The owner's bounded inbox projection of validated records.
    pub fn request_status(&self) -> Result<serde_json::Value, String> {
        let observed = self.observe_request().map_err(|error| error.to_string())?;
        Ok(
            serde_json::to_value(magistr_workflow::request::application::RequestInbox::from(
                &observed.value,
            ))
            .expect("request inbox contains serializable domain fields"),
        )
    }

    pub fn request(&self, id: &str) -> Result<Option<Request>, String> {
        Ok(self
            .request_list()?
            .into_iter()
            .find(|request| request.id == id))
    }

    pub fn request_list(&self) -> Result<Vec<Request>, String> {
        Ok(self
            .observe_request()
            .map_err(|error| error.to_string())?
            .value
            .requests)
    }
}

#[cfg(test)]
#[path = "native_reader.tests.rs"]
mod tests;
