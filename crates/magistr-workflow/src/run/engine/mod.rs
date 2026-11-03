mod admission;
#[cfg(test)]
#[path = "durability.tests.rs"]
mod durability_tests;
mod mutations;
mod transaction;

use crate::cloud::client::CloudClient;
use crate::run::broadcast::{BroadcastFrame, Broadcaster};
use crate::run::check_result_ledger::CheckResultLedger;
use crate::run::domain::execution_identity::ExecutionIdentity;
use crate::run::domain::goal::Goal;
use crate::run::domain::step::{Step, StepStatus};
use crate::run::persistence::Persistence;
use crate::run::saved_cycle::SavedCycle;
use magistr_workflow_contracts::{Observed, ReadError, ReadSource, RunData};

pub struct RunEngine {
    pub(crate) binding: Option<crate::run::application::ExecutionBinding>,
    pub goal: Option<Goal>,
    pub steps: Vec<Step>,
    pub project_id: String,
    pub(crate) execution: ExecutionIdentity,
    next_progress_id: u32,
    saved_cycles: Vec<SavedCycle>,
    selection_revision: u64,
    persistence: Option<Persistence>,
    /// Source of the last acknowledged save or adopted read. Observations
    /// validate the selected file independently of this writer's memory.
    observation: magistr_workflow_contracts::ReadState<()>,
    /// Durable check check_results for this project, outliving the cycle.
    check_results: CheckResultLedger,
    broadcaster: Option<Broadcaster>,
    /// Optional git-native trace mirror. When set, every mutation is
    /// mirrored into `.magistr/traces/` as an Agent Trace JSONL record and the
    /// session is committed on `run_finish`. All git/IO runs on the worker's
    /// own thread, never under this engine's lock. `None` = the default Local
    /// behaviour. Writes are fire-and-forget — a sink error never fails a tool.
    mirror: Option<crate::infra::MirrorWorker>,
    /// Whether mirrored records are `shared` (committed `sessions/`) vs `local`
    /// (gitignored). Default `false`. Only meaningful with `mirror`.
    repo_shared: bool,
    /// Optional cloud sync client. When set, an `ProgressRecorded`
    /// mutation fire-and-forget pushes the progress envelope to the cloud backend.
    /// `None` (default) = no cloud sync.
    cloud: Option<CloudClient>,
}

impl RunEngine {
    pub fn new(project_id: String) -> Self {
        Self {
            binding: None,
            goal: None,
            steps: Vec::new(),
            project_id,
            execution: ExecutionIdentity::fresh(),
            next_progress_id: 1,
            saved_cycles: Vec::new(),
            selection_revision: 0,
            persistence: None,
            observation: magistr_workflow_contracts::ReadState::Current(Observed {
                value: (),
                source: ReadSource::Memory,
            }),
            check_results: CheckResultLedger::in_memory(),
            broadcaster: None,
            mirror: None,
            repo_shared: false,
            cloud: None,
        }
    }

    #[must_use]
    pub fn with_execution_binding(
        mut self,
        binding: crate::run::application::ExecutionBinding,
    ) -> Self {
        self.binding = Some(binding);
        self
    }

    pub fn with_broadcaster(mut self, broadcaster: Broadcaster) -> Self {
        self.set_broadcaster(broadcaster);
        self
    }

    /// Attach the admitted owner's publisher to an already-open engine.
    pub fn set_broadcaster(&mut self, broadcaster: Broadcaster) {
        self.broadcaster = Some(broadcaster);
    }

    /// Attach a cloud client so an `ProgressRecorded` mutation fire-and-forget
    /// pushes the progress envelope to the cloud backend. Wired by
    /// `cli::build_unified`.
    pub fn with_cloud(mut self, client: CloudClient) -> Self {
        self.cloud = Some(client);
        self
    }

    /// A clone of the attached cloud client, if any. The check tools take this
    /// and drop the engine lock BEFORE any network await — a check poll must
    /// never hold the engine hostage.
    #[must_use]
    pub fn cloud_client(&self) -> Option<CloudClient> {
        self.cloud.clone()
    }

    /// The id of the currently active step, if any — the check's `part_of` edge.
    #[must_use]
    pub fn active_step_id(&self) -> Option<String> {
        self.steps
            .iter()
            .find(|t| t.status == StepStatus::Active)
            .map(|t| t.id.clone())
    }

    /// The active step's cycle-scoped wire id (`<cycle>.<step>`), the form a
    /// check's `verifies` edge already uses. Local step ids like `verify`
    /// recur across hundreds of cycles, so an edge carrying one can never be
    /// resolved back to the work it paused; the wire id can. None without an
    /// goal `created_at` — a bare local id would be an unresolvable ref,
    /// so no edge at all is the honest fallback.
    #[must_use]
    pub fn active_step_wire_id(&self) -> Option<String> {
        let local = self.active_step_id()?;
        let created = self.goal.as_ref().and_then(|o| o.created_at.clone())?;
        let cycle = crate::cloud::build::cycle_key(&created);
        Some(format!("{cycle}.{local}"))
    }

    /// Attach a git-native trace sink so mutations are mirrored into the repo's
    /// `.magistr/traces/` as Agent Trace JSONL. `shared` selects the committed
    /// `sessions/` partition (`true`) vs the gitignored `local/` partition
    /// (`false`). Wired by the host's composition root.
    pub fn with_repo_sink(mut self, sink: crate::infra::RepoSink, shared: bool) -> Self {
        self.mirror = Some(crate::infra::MirrorWorker::spawn(sink));
        self.repo_shared = shared;
        self
    }

    /// Block until the git-native mirror has drained every mutation submitted so
    /// far. No-op without a mirror. For graceful shutdown and deterministic tests.
    pub fn flush_mirror(&self) {
        if let Some(m) = &self.mirror {
            m.flush();
        }
    }

    fn broadcast(&self, frame: BroadcastFrame) {
        // Mirror into the git-native trace first, push to the cloud,
        // then fan out to the socket. All fire-and-forget.
        self.mirror_frame_to_repo(&frame);
        self.cloud_push_frame(&frame);
        if let Some(b) = &self.broadcaster {
            b.emit(frame);
        }
    }

    /// Fire-and-forget cloud push of a run mutation frame (sync-run-full —
    /// all four kinds). Identity is cycle-scoped (`cloud::build::cycle_key`
    /// from the goal's `created_at`), so wire ids never collide across
    /// cycles. Store-aware: id-only step frames push the FULL current step
    /// state; goal frames push the current goal (re-push on
    /// `RunFinished` updates the same record in place). No-op without a
    /// cloud client, a current goal `created_at`, OR a tokio runtime (so
    /// sync unit tests never panic); a push error is logged and dropped.
    fn cloud_push_frame(&self, frame: &BroadcastFrame) {
        let Some(client) = &self.cloud else {
            return;
        };
        let Some(cycle_created) = self.goal.as_ref().and_then(|o| o.created_at.clone()) else {
            return;
        };
        let cycle = crate::cloud::build::cycle_key(&cycle_created);
        let envelope = match frame {
            BroadcastFrame::ProgressRecorded { progress, .. } => {
                crate::cloud::build::from_progress(&self.project_id, &cycle, progress)
            }
            BroadcastFrame::GoalSet { goal } => {
                crate::cloud::build::from_goal(&self.project_id, &cycle, goal)
            }
            BroadcastFrame::RunFinished { .. } => match &self.goal {
                Some(goal) => crate::cloud::build::from_goal(&self.project_id, &cycle, goal),
                None => return,
            },
            BroadcastFrame::StepAdded { step_id, .. }
            | BroadcastFrame::StepStarted { step_id }
            | BroadcastFrame::StepWaiting { step_id, .. } => {
                match self.steps.iter().find(|t| t.id == *step_id) {
                    Some(step) => crate::cloud::build::from_run_step(
                        &self.project_id,
                        &cycle,
                        &cycle_created,
                        step,
                    ),
                    None => return,
                }
            }
            BroadcastFrame::StepDone { step } => {
                crate::cloud::build::from_run_step(&self.project_id, &cycle, &cycle_created, step)
            }
            BroadcastFrame::CheckRecorded { step_id, check } => {
                // Identity = the check's append-only index within its step;
                // the just-recorded check is the last one.
                let Some(step) = self.steps.iter().find(|t| t.id == *step_id) else {
                    return;
                };
                let seq = step.checks.len().saturating_sub(1);
                crate::cloud::build::from_check(&self.project_id, &cycle, step_id, seq, check)
            }
            BroadcastFrame::Cleared => return,
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let client = client.clone();
        handle.spawn(async move {
            if let Err(e) = client.push(&envelope).await {
                tracing::warn!(target: "magistr_workflow::cloud", "run cloud push failed: {e}");
            }
        });
    }

    /// Map a mutation frame to an Agent Trace record and append it to the repo
    /// trace; commit the session on `RunFinished`. No-op without a sink.
    /// Fire-and-forget: every failure is logged at WARN and dropped so the
    /// mutation path is never affected. The frame→record mapping lives here
    /// (engine-side) so `infra::repo_sync` stays domain-free.
    fn mirror_frame_to_repo(&self, frame: &BroadcastFrame) {
        let Some(mirror) = &self.mirror else {
            return;
        };

        let step_payload = |step_id: &str| {
            self.steps
                .iter()
                .find(|t| t.id == step_id)
                .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null)
        };

        // (kind, payload, files[], is_session_close)
        let (kind, payload, files, closes) = match frame {
            BroadcastFrame::GoalSet { goal } => (
                "goal",
                serde_json::to_value(goal).unwrap_or(serde_json::Value::Null),
                vec![],
                false,
            ),
            BroadcastFrame::StepAdded { step_id, .. }
            | BroadcastFrame::StepStarted { step_id }
            | BroadcastFrame::StepWaiting { step_id, .. } => {
                ("step", step_payload(step_id), vec![], false)
            }
            BroadcastFrame::StepDone { step } => ("step", step_payload(&step.id), vec![], false),
            BroadcastFrame::ProgressRecorded { progress, .. } => {
                let model_id = std::env::var("MAGISTR_WORKFLOW_MODEL_ID")
                    .ok()
                    .filter(|s| !s.is_empty());
                let files = progress
                    .files_touched
                    .iter()
                    .map(|p| crate::infra::file_attribution(p, model_id.as_deref()))
                    .collect();
                (
                    "progress",
                    serde_json::to_value(progress).unwrap_or(serde_json::Value::Null),
                    files,
                    false,
                )
            }
            BroadcastFrame::CheckRecorded { check, .. } => (
                "check",
                serde_json::to_value(check).unwrap_or(serde_json::Value::Null),
                vec![],
                false,
            ),
            BroadcastFrame::RunFinished { .. } => (
                "goal",
                serde_json::to_value(&self.goal).unwrap_or(serde_json::Value::Null),
                vec![],
                true,
            ),
            // `Cleared` is a reset, not a trace event — nothing to record.
            BroadcastFrame::Cleared => return,
        };

        // Hand off to the worker thread; building the record (which shells git),
        // the append, and any commit all happen off this engine's lock.
        mirror.submit(crate::infra::MirrorJob {
            family: "run",
            kind,
            session_id: self.project_id.clone(),
            shared: self.repo_shared,
            payload,
            files,
            closes,
        });
    }

    pub fn with_persistence(mut self, persistence: Persistence) -> Self {
        self.check_results = persistence.check_result_ledger(&self.project_id);
        self.persistence = Some(persistence);
        self.observation = magistr_workflow_contracts::ReadState::Unobserved;
        if let Err(error) = self.reload_from_store() {
            tracing::warn!(%error, "The saved work could not be loaded");
        }
        self
    }

    /// Read this execution's stored records without changing live writer state.
    pub fn observe(&self) -> Result<Observed<RunData>, ReadError> {
        let Some(persistence) = self.persistence.as_ref().filter(|p| p.enabled()) else {
            return Ok(Observed {
                value: self.persisted_state().into_observation(),
                source: ReadSource::Memory,
            });
        };
        let read = persistence.current_stamped(&self.project_id)?;
        let value = read
            .value
            .map_or_else(RunData::default, |state| state.into_observation());
        Ok(Observed {
            value,
            source: read.source,
        })
    }

    /// The durable check_result ledger this engine settles checks into. Cloned
    /// by the use cases so admission can publish a check_result without holding
    /// this engine's lock.
    #[must_use]
    pub fn check_results(&self) -> &CheckResultLedger {
        &self.check_results
    }

    pub(crate) fn compact_check_results(
        &self,
        retained: impl FnOnce() -> Result<std::collections::BTreeSet<String>, String>,
    ) -> Result<
        crate::run::check_result_compaction::CheckResultCompaction,
        crate::run::application::RunError,
    > {
        if let Some(persistence) = &self.persistence
            && persistence.enabled()
        {
            return persistence.compact_check_results(
                &self.project_id,
                &self.execution.run_id,
                retained,
            );
        }
        if self.goal.is_none() {
            return Err("Open a run before clearing old check history.".into());
        }
        let saved_check_results: std::collections::BTreeSet<_> = self
            .saved_cycles
            .iter()
            .map(|cycle| self.check_results.for_run(&cycle.execution.run_id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .map(|check_result| check_result.id)
            .collect();
        self.check_results.compact(&self.execution.run_id, || {
            let mut ids = retained()?;
            ids.extend(saved_check_results);
            Ok(ids)
        })
    }

    /// The revision of the current goal's acceptance, `None` without one.
    #[must_use]
    pub fn acceptance_revision(&self) -> Option<String> {
        self.goal.as_ref().map(Goal::acceptance_revision)
    }

    /// JSON status for export clients.
    pub fn status(&self) -> serde_json::Value {
        serde_json::to_value(self.snapshot().status()).expect("serializable run status")
    }

    pub fn snapshot(&self) -> crate::run::application::RunSnapshot {
        crate::run::application::RunSnapshot::from_engine(self)
    }

    pub fn export(&self, format: &str) -> String {
        match format {
            "json" => serde_json::to_string_pretty(&self.status()).unwrap_or_default(),
            _ => self.export_markdown(),
        }
    }

    pub fn plan_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "steps": self.steps.iter().map(|t| serde_json::json!({
                "id": t.id,
                "title": t.title,
                "type": t.step_type,
                "status": t.status,
                "estimate": t.estimate,
            })).collect::<Vec<_>>(),
            "total": self.steps.len(),
        })
    }

    fn export_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Execution Trace\n\n");

        if let Some(obj) = &self.goal {
            out.push_str(&format!("## Goal: {}\n", obj.description));
            out.push_str(&format!("Status: {:?}\n\n", obj.status));
            if !obj.acceptance_criteria.is_empty() {
                out.push_str("### Acceptance Criteria\n");
                for c in &obj.acceptance_criteria {
                    out.push_str(&format!("- {c}\n"));
                }
                out.push('\n');
            }
        }

        out.push_str("## Steps\n\n");
        for step in &self.steps {
            let status_icon = match step.status {
                StepStatus::Done => "[x]",
                StepStatus::Active => "[>]",
                StepStatus::Waiting => "[!]",
                StepStatus::Skipped => "[-]",
                StepStatus::Planned => "[ ]",
            };
            out.push_str(&format!(
                "- {status_icon} **{}** ({})\n",
                step.title, step.id
            ));

            for progress in &step.progress {
                let step_ref = progress
                    .note
                    .map(|s| format!(" (decision #{s})"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  - {:?}: {}{}\n",
                    progress.progress_type, progress.description, step_ref
                ));
            }
            for check in &step.checks {
                let icon = if check.passed { "pass" } else { "FAIL" };
                let req = if check.required { " (required)" } else { "" };
                out.push_str(&format!("  - [{icon}] {}{req}\n", check.name));
            }
            for artifact in &step.artifacts {
                out.push_str(&format!(
                    "  - artifact: {:?} {}\n",
                    artifact.artifact_type, artifact.reference
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod wire_id_tests {
    use super::*;
    use crate::run::domain::step::StepType;

    /// The check edge must carry the cycle-scoped form a check edge uses —
    /// a bare local id like `verify` recurs across cycles and resolves to
    /// nothing.
    #[test]
    fn active_step_wire_id_is_cycle_scoped() {
        let mut engine = RunEngine::new("magistr-test".into());
        engine
            .set_goal("task:x — test".into(), vec![], vec![], String::new())
            .unwrap();
        engine
            .add_step(
                "verify".into(),
                "Verify".into(),
                StepType::Review,
                None,
                None,
            )
            .unwrap();
        engine.start_step("verify").expect("step starts");

        let wire = engine
            .active_step_wire_id()
            .expect("active step has a wire id");
        let created = engine
            .goal
            .as_ref()
            .and_then(|o| o.created_at.clone())
            .expect("goal is stamped");
        let cycle = crate::cloud::build::cycle_key(&created);
        assert_eq!(wire, format!("{cycle}.verify"));
        assert_ne!(wire, "verify", "the local id alone is not a resolvable ref");
    }

    /// With no active step there is no ref to carry — the edge is omitted
    /// rather than invented.
    #[test]
    fn no_active_step_means_no_wire_id() {
        let mut engine = RunEngine::new("magistr-test".into());
        engine
            .set_goal("task:x — test".into(), vec![], vec![], String::new())
            .unwrap();
        assert_eq!(engine.active_step_wire_id(), None);
    }
}
