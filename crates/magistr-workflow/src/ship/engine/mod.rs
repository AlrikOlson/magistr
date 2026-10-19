mod admission;
#[cfg(test)]
#[path = "durability.tests.rs"]
mod durability_tests;
mod mutations;
mod transaction;

use crate::cloud::client::CloudClient;
use crate::ship::broadcast::{BroadcastFrame, Broadcaster};
use crate::ship::domain::execution_identity::ExecutionIdentity;
use crate::ship::domain::objective::Objective;
use crate::ship::domain::task::{Task, TaskStatus};
use crate::ship::persistence::Persistence;
use crate::ship::receipt_ledger::ReceiptLedger;
use crate::ship::saved_cycle::SavedCycle;
use magistr_workflow_contracts::{Observed, ReadError, ReadSource, ShipData};

pub struct ShipEngine {
    pub(crate) binding: Option<crate::ship::application::ExecutionBinding>,
    pub objective: Option<Objective>,
    pub tasks: Vec<Task>,
    pub project_id: String,
    pub(crate) execution: ExecutionIdentity,
    next_action_id: u32,
    saved_cycles: Vec<SavedCycle>,
    selection_revision: u64,
    persistence: Option<Persistence>,
    /// Source of the last acknowledged save or adopted read. Observations
    /// validate the selected file independently of this writer's memory.
    observation: magistr_workflow_contracts::ReadState<()>,
    /// Durable check receipts for this project, outliving the cycle.
    receipts: ReceiptLedger,
    broadcaster: Option<Broadcaster>,
    /// Optional git-native trace mirror. When set, every mutation is
    /// mirrored into `.magistr/traces/` as an Agent Trace JSONL record and the
    /// session is committed on `ship_finalize`. All git/IO runs on the worker's
    /// own thread, never under this engine's lock. `None` = the default Local
    /// behaviour. Writes are fire-and-forget — a sink error never fails a tool.
    mirror: Option<crate::infra::MirrorWorker>,
    /// Whether mirrored records are `shared` (committed `sessions/`) vs `local`
    /// (gitignored). Default `false`. Only meaningful with `mirror`.
    repo_shared: bool,
    /// Optional cloud sync client. When set, an `ActionRecorded`
    /// mutation fire-and-forget pushes the action envelope to the cloud backend.
    /// `None` (default) = no cloud sync.
    cloud: Option<CloudClient>,
}

impl ShipEngine {
    pub fn new(project_id: String) -> Self {
        Self {
            binding: None,
            objective: None,
            tasks: Vec::new(),
            project_id,
            execution: ExecutionIdentity::fresh(),
            next_action_id: 1,
            saved_cycles: Vec::new(),
            selection_revision: 0,
            persistence: None,
            observation: magistr_workflow_contracts::ReadState::Current(Observed {
                value: (),
                source: ReadSource::Memory,
            }),
            receipts: ReceiptLedger::in_memory(),
            broadcaster: None,
            mirror: None,
            repo_shared: false,
            cloud: None,
        }
    }

    #[must_use]
    pub fn with_execution_binding(
        mut self,
        binding: crate::ship::application::ExecutionBinding,
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

    /// Attach a cloud client so an `ActionRecorded` mutation fire-and-forget
    /// pushes the action envelope to the cloud backend. Wired by
    /// `cli::build_unified`.
    pub fn with_cloud(mut self, client: CloudClient) -> Self {
        self.cloud = Some(client);
        self
    }

    /// A clone of the attached cloud client, if any. The gate tools take this
    /// and drop the engine lock BEFORE any network await — a gate poll must
    /// never hold the engine hostage.
    #[must_use]
    pub fn cloud_client(&self) -> Option<CloudClient> {
        self.cloud.clone()
    }

    /// The id of the currently active task, if any — the gate's `part_of` edge.
    #[must_use]
    pub fn active_task_id(&self) -> Option<String> {
        self.tasks
            .iter()
            .find(|t| t.status == TaskStatus::Active)
            .map(|t| t.id.clone())
    }

    /// The active task's cycle-scoped wire id (`<cycle>.<task>`), the form a
    /// check's `verifies` edge already uses. Local task ids like `verify`
    /// recur across hundreds of cycles, so an edge carrying one can never be
    /// resolved back to the work it paused; the wire id can. None without an
    /// objective `created_at` — a bare local id would be an unresolvable ref,
    /// so no edge at all is the honest fallback.
    #[must_use]
    pub fn active_task_wire_id(&self) -> Option<String> {
        let local = self.active_task_id()?;
        let created = self.objective.as_ref().and_then(|o| o.created_at.clone())?;
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

    /// Fire-and-forget cloud push of a ship mutation frame (sync-ship-full —
    /// all four kinds). Identity is cycle-scoped (`cloud::build::cycle_key`
    /// from the objective's `created_at`), so wire ids never collide across
    /// cycles. Store-aware: id-only task frames push the FULL current task
    /// state; objective frames push the current objective (re-push on
    /// `ObjectiveShipped` updates the same record in place). No-op without a
    /// cloud client, a current objective `created_at`, OR a tokio runtime (so
    /// sync unit tests never panic); a push error is logged and dropped.
    fn cloud_push_frame(&self, frame: &BroadcastFrame) {
        let Some(client) = &self.cloud else {
            return;
        };
        let Some(cycle_created) = self.objective.as_ref().and_then(|o| o.created_at.clone()) else {
            return;
        };
        let cycle = crate::cloud::build::cycle_key(&cycle_created);
        let envelope = match frame {
            BroadcastFrame::ActionRecorded { action, .. } => {
                crate::cloud::build::from_action(&self.project_id, &cycle, action)
            }
            BroadcastFrame::ObjectiveSet { objective } => {
                crate::cloud::build::from_objective(&self.project_id, &cycle, objective)
            }
            BroadcastFrame::ObjectiveShipped { .. } => match &self.objective {
                Some(objective) => {
                    crate::cloud::build::from_objective(&self.project_id, &cycle, objective)
                }
                None => return,
            },
            BroadcastFrame::TaskAdded { task_id, .. }
            | BroadcastFrame::TaskStarted { task_id }
            | BroadcastFrame::TaskBlocked { task_id, .. } => {
                match self.tasks.iter().find(|t| t.id == *task_id) {
                    Some(task) => crate::cloud::build::from_task(
                        &self.project_id,
                        &cycle,
                        &cycle_created,
                        task,
                    ),
                    None => return,
                }
            }
            BroadcastFrame::TaskCompleted { task } => {
                crate::cloud::build::from_task(&self.project_id, &cycle, &cycle_created, task)
            }
            BroadcastFrame::CheckRecorded { task_id, check } => {
                // Identity = the check's append-only index within its task;
                // the just-recorded check is the last one.
                let Some(task) = self.tasks.iter().find(|t| t.id == *task_id) else {
                    return;
                };
                let seq = task.checks.len().saturating_sub(1);
                crate::cloud::build::from_check(&self.project_id, &cycle, task_id, seq, check)
            }
            BroadcastFrame::Cleared => return,
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let client = client.clone();
        handle.spawn(async move {
            if let Err(e) = client.push(&envelope).await {
                tracing::warn!(target: "magistr_workflow::cloud", "ship cloud push failed: {e}");
            }
        });
    }

    /// Map a mutation frame to an Agent Trace record and append it to the repo
    /// trace; commit the session on `ObjectiveShipped`. No-op without a sink.
    /// Fire-and-forget: every failure is logged at WARN and dropped so the
    /// mutation path is never affected. The frame→record mapping lives here
    /// (engine-side) so `infra::repo_sync` stays domain-free.
    fn mirror_frame_to_repo(&self, frame: &BroadcastFrame) {
        let Some(mirror) = &self.mirror else {
            return;
        };

        let task_payload = |task_id: &str| {
            self.tasks
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
                .unwrap_or(serde_json::Value::Null)
        };

        // (kind, payload, files[], is_session_close)
        let (kind, payload, files, closes) = match frame {
            BroadcastFrame::ObjectiveSet { objective } => (
                "objective",
                serde_json::to_value(objective).unwrap_or(serde_json::Value::Null),
                vec![],
                false,
            ),
            BroadcastFrame::TaskAdded { task_id, .. }
            | BroadcastFrame::TaskStarted { task_id }
            | BroadcastFrame::TaskBlocked { task_id, .. } => {
                ("task", task_payload(task_id), vec![], false)
            }
            BroadcastFrame::TaskCompleted { task } => {
                ("task", task_payload(&task.id), vec![], false)
            }
            BroadcastFrame::ActionRecorded { action, .. } => {
                let model_id = std::env::var("MAGISTR_WORKFLOW_MODEL_ID")
                    .ok()
                    .filter(|s| !s.is_empty());
                let files = action
                    .files_touched
                    .iter()
                    .map(|p| crate::infra::file_attribution(p, model_id.as_deref()))
                    .collect();
                (
                    "action",
                    serde_json::to_value(action).unwrap_or(serde_json::Value::Null),
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
            BroadcastFrame::ObjectiveShipped { .. } => (
                "objective",
                serde_json::to_value(&self.objective).unwrap_or(serde_json::Value::Null),
                vec![],
                true,
            ),
            // `Cleared` is a reset, not a trace event — nothing to record.
            BroadcastFrame::Cleared => return,
        };

        // Hand off to the worker thread; building the record (which shells git),
        // the append, and any commit all happen off this engine's lock.
        mirror.submit(crate::infra::MirrorJob {
            family: "ship",
            kind,
            session_id: self.project_id.clone(),
            shared: self.repo_shared,
            payload,
            files,
            closes,
        });
    }

    pub fn with_persistence(mut self, persistence: Persistence) -> Self {
        self.receipts = persistence.receipt_ledger(&self.project_id);
        self.persistence = Some(persistence);
        self.observation = magistr_workflow_contracts::ReadState::Unobserved;
        if let Err(error) = self.reload_from_store() {
            tracing::warn!(%error, "The saved work could not be loaded");
        }
        self
    }

    /// Read this execution's stored records without changing live writer state.
    pub fn observe(&self) -> Result<Observed<ShipData>, ReadError> {
        let Some(persistence) = self.persistence.as_ref().filter(|p| p.enabled()) else {
            return Ok(Observed {
                value: self.persisted_state().into_observation(),
                source: ReadSource::Memory,
            });
        };
        let read = persistence.current_stamped(&self.project_id)?;
        let value = read
            .value
            .map_or_else(ShipData::default, |state| state.into_observation());
        Ok(Observed {
            value,
            source: read.source,
        })
    }

    /// The durable receipt ledger this engine settles checks into. Cloned
    /// by the use cases so admission can publish a receipt without holding
    /// this engine's lock.
    #[must_use]
    pub fn receipts(&self) -> &ReceiptLedger {
        &self.receipts
    }

    pub(crate) fn compact_receipts(
        &self,
        retained: impl FnOnce() -> Result<std::collections::BTreeSet<String>, String>,
    ) -> Result<
        crate::ship::receipt_compaction::ReceiptCompaction,
        crate::ship::application::ShipError,
    > {
        if let Some(persistence) = &self.persistence
            && persistence.enabled()
        {
            return persistence.compact_receipts(
                &self.project_id,
                &self.execution.run_id,
                retained,
            );
        }
        if self.objective.is_none() {
            return Err("Set an objective before clearing old check history.".into());
        }
        let saved_receipts: std::collections::BTreeSet<_> = self
            .saved_cycles
            .iter()
            .map(|cycle| self.receipts.for_run(&cycle.execution.run_id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .map(|receipt| receipt.id)
            .collect();
        self.receipts.compact(&self.execution.run_id, || {
            let mut ids = retained()?;
            ids.extend(saved_receipts);
            Ok(ids)
        })
    }

    /// The revision of the current objective's acceptance, `None` without one.
    #[must_use]
    pub fn acceptance_revision(&self) -> Option<String> {
        self.objective.as_ref().map(Objective::acceptance_revision)
    }

    /// JSON status for export clients.
    pub fn status(&self) -> serde_json::Value {
        serde_json::to_value(self.snapshot().status()).expect("serializable ship status")
    }

    pub fn snapshot(&self) -> crate::ship::application::ShipSnapshot {
        crate::ship::application::ShipSnapshot::from_engine(self)
    }

    pub fn export(&self, format: &str) -> String {
        match format {
            "json" => serde_json::to_string_pretty(&self.status()).unwrap_or_default(),
            _ => self.export_markdown(),
        }
    }

    pub fn plan_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "tasks": self.tasks.iter().map(|t| serde_json::json!({
                "id": t.id,
                "title": t.title,
                "type": t.task_type,
                "status": t.status,
                "estimate": t.estimate,
            })).collect::<Vec<_>>(),
            "total": self.tasks.len(),
        })
    }

    fn export_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Execution Trace\n\n");

        if let Some(obj) = &self.objective {
            out.push_str(&format!("## Objective: {}\n", obj.description));
            out.push_str(&format!("Status: {:?}\n\n", obj.status));
            if !obj.acceptance_criteria.is_empty() {
                out.push_str("### Acceptance Criteria\n");
                for c in &obj.acceptance_criteria {
                    out.push_str(&format!("- {c}\n"));
                }
                out.push('\n');
            }
        }

        out.push_str("## Tasks\n\n");
        for task in &self.tasks {
            let status_icon = match task.status {
                TaskStatus::Completed => "[x]",
                TaskStatus::Active => "[>]",
                TaskStatus::Blocked => "[!]",
                TaskStatus::Skipped => "[-]",
                TaskStatus::Planned => "[ ]",
            };
            out.push_str(&format!(
                "- {status_icon} **{}** ({})\n",
                task.title, task.id
            ));

            for action in &task.actions {
                let step_ref = action
                    .think_step
                    .map(|s| format!(" (think #{s})"))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  - {:?}: {}{}\n",
                    action.action_type, action.description, step_ref
                ));
            }
            for check in &task.checks {
                let icon = if check.passed { "pass" } else { "FAIL" };
                let req = if check.required { " (required)" } else { "" };
                out.push_str(&format!("  - [{icon}] {}{req}\n", check.name));
            }
            for artifact in &task.artifacts {
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
    use crate::ship::domain::task::TaskType;

    /// The gate edge must carry the cycle-scoped form a check edge uses —
    /// a bare local id like `verify` recurs across cycles and resolves to
    /// nothing.
    #[test]
    fn active_task_wire_id_is_cycle_scoped() {
        let mut engine = ShipEngine::new("magistr-test".into());
        engine
            .set_objective("chunk:x — test".into(), vec![], vec![], String::new())
            .unwrap();
        engine
            .add_task(
                "verify".into(),
                "Verify".into(),
                TaskType::Review,
                None,
                None,
            )
            .unwrap();
        engine.start_task("verify").expect("task starts");

        let wire = engine
            .active_task_wire_id()
            .expect("active task has a wire id");
        let created = engine
            .objective
            .as_ref()
            .and_then(|o| o.created_at.clone())
            .expect("objective is stamped");
        let cycle = crate::cloud::build::cycle_key(&created);
        assert_eq!(wire, format!("{cycle}.verify"));
        assert_ne!(wire, "verify", "the local id alone is not a resolvable ref");
    }

    /// With no active task there is no ref to carry — the edge is omitted
    /// rather than invented.
    #[test]
    fn no_active_task_means_no_wire_id() {
        let mut engine = ShipEngine::new("magistr-test".into());
        engine
            .set_objective("chunk:x — test".into(), vec![], vec![], String::new())
            .unwrap();
        assert_eq!(engine.active_task_wire_id(), None);
    }
}
