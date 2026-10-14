use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::infra::{MergedWrite, StoreStamp, read_json, store_stamp};
use crate::ship::domain::execution_identity::ExecutionIdentity;
use crate::ship::domain::objective::Objective;
use crate::ship::domain::task::Task;
use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};

use super::saved_cycle::SavedCycle;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    schema_version: u32,
    project_id: String,
    pub(crate) objective: Option<Objective>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) next_action_id: u32,
    pub(crate) execution: ExecutionIdentity,
    pub(crate) selection_revision: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) saved_cycles: Vec<SavedCycle>,
}

impl PersistedState {
    fn is_complete(&self) -> bool {
        self.execution.has_task_instances(&self.tasks)
            && self
                .saved_cycles
                .iter()
                .all(|cycle| cycle.execution.has_task_instances(&cycle.tasks))
    }

    /// The identity of the cycle: the objective's `created_at`.
    pub(crate) fn cycle_stamp(&self) -> Option<&str> {
        self.objective
            .as_ref()
            .and_then(|o| o.created_at.as_deref())
    }
}

/// Merge policy for the locked merge-on-save discipline
/// (family-stores-merge-on-save). Ship state is CYCLE-shaped — one objective
/// plus its tasks — so the cycle identity (`objective.created_at`) decides:
///
/// - **Track selection**: a later selection revision wins, including its saved
///   plans. A stale writer cannot reselect an earlier plan.
/// - **Different cycles at the same revision**: the newer cycle wins outright. A stale process
///   still scribbling into a cycle another process has reset away loses by
///   design — that is exactly what keeps `reset` resurrection-proof against
///   stale writers. A state with no objective counts as oldest.
/// - **Same cycle** (identical `created_at`): tasks union by id (disk-only
///   tasks another process acked survive; memory wins per shared task — the
///   LWW caveat the think merge also accepted) and `next_action_id` takes
///   the max so action ids never regress.
fn merge_states(memory: &PersistedState, disk: PersistedState) -> PersistedState {
    let mem_stamp = memory.cycle_stamp();
    let disk_stamp = disk.cycle_stamp();
    if mem_stamp == disk_stamp && memory.selection_revision == disk.selection_revision {
        return merge_same_cycle(memory, disk);
    }
    let newer = |a: &str, b: &str| {
        use chrono::DateTime;
        match (
            DateTime::parse_from_rfc3339(a).ok(),
            DateTime::parse_from_rfc3339(b).ok(),
        ) {
            (Some(a), Some(b)) => a > b,
            (Some(_), None) => true,
            _ => false,
        }
    };
    let disk_is_newer = if disk.selection_revision != memory.selection_revision {
        disk.selection_revision > memory.selection_revision
    } else {
        match (disk_stamp, mem_stamp) {
            (Some(d), Some(m)) => newer(d, m),
            (Some(_), None) => true,
            _ => false,
        }
    };
    if disk_is_newer { disk } else { memory.clone() }
}

fn merge_same_cycle(memory: &PersistedState, disk: PersistedState) -> PersistedState {
    if !memory.is_complete() || !disk.is_complete() {
        tracing::warn!("Ship state is incomplete; the write was refused");
        return disk;
    }
    let memory_identity = &memory.execution;
    let disk_identity = &disk.execution;
    let mut identity = memory.execution.clone();
    identity
        .removed_instances
        .extend(disk_identity.removed_instances.iter().cloned());
    identity.tasks.clear();
    let mut tasks: Vec<Task> = Vec::new();
    for (source, identities) in [
        (&memory.tasks, memory_identity),
        (&disk.tasks, disk_identity),
    ] {
        for task in source {
            let instance = identities
                .tasks
                .get(&task.id)
                .expect("complete state has an instance for every task")
                .clone();
            if identity.removed_instances.contains(&instance)
                || tasks.iter().any(|saved| saved.id == task.id)
            {
                continue;
            }
            identity.tasks.insert(task.id.clone(), instance);
            tasks.push(task.clone());
        }
    }
    PersistedState {
        schema_version: memory.schema_version,
        project_id: memory.project_id.clone(),
        objective: memory.objective.clone(),
        tasks,
        next_action_id: memory.next_action_id.max(disk.next_action_id),
        saved_cycles: memory.saved_cycles.clone(),
        selection_revision: memory.selection_revision,
        execution: identity,
    }
}

#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    pub enabled: bool,
    pub data_dir: PathBuf,
}

impl PersistenceConfig {
    pub fn from_env() -> Self {
        let enabled = env::var("THINK_AND_SHIP_PERSIST")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let data_dir = env::var("THINK_AND_SHIP_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_data_dir());

        Self { enabled, data_dir }
    }
}

fn default_data_dir() -> PathBuf {
    crate::infra::persistence::default_data_dir()
}

#[derive(Debug, Clone)]
pub struct Persistence {
    enabled: bool,
    sessions_dir: PathBuf,
}

impl Persistence {
    /// Validate the selected execution file and retain its exact source.
    pub(crate) fn current_stamped(
        &self,
        project_id: &str,
    ) -> Result<Observed<Option<PersistedState>>, ReadError> {
        if !self.enabled {
            return Ok(Observed {
                value: None,
                source: ReadSource::Memory,
            });
        }
        let path = self.path_for(project_id);
        let read: Observed<Option<PersistedState>> = read_json(&path)?;
        if let Some(state) = &read.value {
            if state.schema_version != SCHEMA_VERSION {
                return Err(ReadError::new(
                    ReadErrorKind::Schema,
                    "The saved work uses an unsupported format.",
                ));
            }
            if state.project_id != project_id
                || state
                    .objective
                    .as_ref()
                    .is_some_and(|objective| objective.project_id != project_id)
                || state
                    .saved_cycles
                    .iter()
                    .any(|cycle| cycle.objective.project_id != project_id)
            {
                return Err(ReadError::new(
                    ReadErrorKind::Identity,
                    "The saved work could not be matched to this project.",
                ));
            }
            if !state.is_complete() {
                return Err(ReadError::new(
                    ReadErrorKind::Identity,
                    "The saved work contains incomplete or duplicate tasks.",
                ));
            }
        }
        Ok(read)
    }

    fn locked<T>(
        &self,
        project_id: &str,
        operation: impl FnOnce() -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        fs::create_dir_all(&self.sessions_dir)?;
        let path = self.sessions_dir.join(format!("{project_id}.json.lock"));
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)?;
        lock.lock()?;
        // The owned file releases the same lock used by ordinary merge writes
        // on every return path. Never unlink the lock inode.
        operation()
    }

    /// Append `check` to its admitted task instance under the file lock,
    /// answering the cycle as it stands afterwards, whether the check
    /// applied, and the stamp of the file that cycle was read from or
    /// written to.
    pub(crate) fn settle_check(
        &self,
        target: &crate::ship::application::TaskAdmission,
        check: &crate::ship::domain::check::Check,
    ) -> std::io::Result<(Option<PersistedState>, bool, Option<StoreStamp>)> {
        self.locked(&target.project_id, || {
            let read = self
                .current_stamped(&target.project_id)
                .map_err(std::io::Error::other)?;
            let current = read.value;
            let stamp = match read.source {
                ReadSource::File(stamp) => Some(stamp),
                ReadSource::Memory | ReadSource::Missing => None,
            };
            let Some(mut state) = current else {
                return Ok((None, false, None));
            };
            let identity = &state.execution;
            let matches = identity.run_id == target.run_id
                && identity.tasks.get(&target.task_id) == Some(&target.task_instance);
            let index = state
                .tasks
                .iter()
                .position(|task| task.id == target.task_id);
            let unique = state
                .tasks
                .iter()
                .filter(|task| task.id == target.task_id)
                .count()
                == 1;
            let Some(index) = index.filter(|_| matches && unique) else {
                return Ok((Some(state), false, stamp));
            };
            state.tasks[index].checks.push(check.clone());
            let path = self
                .sessions_dir
                .join(format!("{}.json", target.project_id));
            let temporary = path.with_extension("json.tmp");
            fs::write(
                &temporary,
                serde_json::to_vec(&state).map_err(std::io::Error::other)?,
            )?;
            fs::rename(temporary, &path)?;
            let stamp = store_stamp::of(&path).ok().flatten();
            Ok((Some(state), true, stamp))
        })
    }

    pub fn new(cfg: &PersistenceConfig) -> Self {
        // Partition under `ship/` so the think family writes to its own
        // sibling subdirectory and the two never share a `<project>.json`
        // path. Mirrors the layout used by `crate::infra::Persistence`.
        let sessions_dir = cfg.data_dir.join("ship").join("sessions");
        if cfg.enabled
            && let Err(e) = fs::create_dir_all(&sessions_dir)
        {
            tracing::warn!(
                "ship: could not create data dir {}: {e}",
                sessions_dir.display()
            );
        }
        Self {
            enabled: cfg.enabled,
            sessions_dir,
        }
    }

    /// Each execution owns its selected cycle and receipt ledger within the project store.
    #[must_use]
    pub fn for_execution(mut self, key: &str) -> Self {
        let ship = self
            .sessions_dir
            .parent()
            .expect("ship sessions have a parent");
        self.sessions_dir = ship.join("executions").join(key).join("sessions");
        self
    }

    /// The file selected by this persistence owner for one project's execution.
    pub fn path_for(&self, project_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{project_id}.json"))
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The receipt ledger beside this store's sessions: on disk under
    /// `ship/receipts/<project>` when persistence is on, in memory otherwise.
    pub(crate) fn receipt_ledger(
        &self,
        project_id: &str,
    ) -> crate::ship::receipt_ledger::ReceiptLedger {
        if !self.enabled {
            return crate::ship::receipt_ledger::ReceiptLedger::in_memory();
        }
        let root = self
            .sessions_dir
            .parent()
            .map_or_else(|| self.sessions_dir.clone(), std::path::Path::to_path_buf);
        crate::ship::receipt_ledger::ReceiptLedger::on_disk(root.join("receipts").join(project_id))
    }

    /// Pin the persisted current run against rollover for the whole pruning pass.
    /// Lock order is cycle then ledger, matching check settlement.
    pub(crate) fn compact_receipts(
        &self,
        project_id: &str,
        retained: impl FnOnce() -> Result<std::collections::BTreeSet<String>, String>,
    ) -> std::io::Result<crate::ship::receipt_compaction::ReceiptCompaction> {
        self.locked(project_id, || {
            let current = self
                .current_stamped(project_id)
                .map_err(std::io::Error::other)?
                .value;
            let state = current.filter(|s| s.objective.is_some()).ok_or_else(|| {
                std::io::Error::other("Set an objective before clearing old check history.")
            })?;
            let identity = state.execution;
            let ledger = self.receipt_ledger(project_id);
            let saved_receipts: std::collections::BTreeSet<_> = state
                .saved_cycles
                .iter()
                .map(|cycle| ledger.for_run(&cycle.execution.run_id))
                .collect::<Result<Vec<_>, _>>()
                .map_err(std::io::Error::other)?
                .into_iter()
                .flatten()
                .map(|receipt| receipt.id)
                .collect();
            ledger
                .compact(&identity.run_id, || {
                    let mut ids = retained()?;
                    ids.extend(saved_receipts);
                    Ok(ids)
                })
                .map_err(std::io::Error::other)
        })
    }

    /// Persist the complete execution record, answering what landed on disk
    /// and the stamp of the written file; `None` when persistence is off
    /// or the write failed (logged).
    // These required fields describe one atomic plan selection; the private
    // serialization boundary keeps identity, tasks and saved plans in one write.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn save_with_identity(
        &self,
        project_id: &str,
        objective: &Option<Objective>,
        tasks: &[Task],
        next_action_id: u32,
        execution: &ExecutionIdentity,
        saved_cycles: &[SavedCycle],
        selection_revision: u64,
    ) -> Option<MergedWrite<PersistedState>> {
        self.save_state(
            project_id,
            objective,
            tasks,
            next_action_id,
            execution.clone(),
            saved_cycles,
            selection_revision,
        )
    }

    // These required fields describe one atomic plan selection; the private
    // serialization boundary keeps identity, tasks and saved plans in one write.
    #[allow(clippy::too_many_arguments)]
    fn save_state(
        &self,
        project_id: &str,
        objective: &Option<Objective>,
        tasks: &[Task],
        next_action_id: u32,
        execution: ExecutionIdentity,
        saved_cycles: &[SavedCycle],
        selection_revision: u64,
    ) -> Option<MergedWrite<PersistedState>> {
        if !self.enabled {
            return None;
        }
        let state = PersistedState {
            schema_version: SCHEMA_VERSION,
            project_id: project_id.to_string(),
            objective: objective.clone(),
            tasks: tasks.to_vec(),
            next_action_id,
            execution,
            saved_cycles: saved_cycles.to_vec(),
            selection_revision,
        };
        if !state.is_complete() {
            tracing::warn!("Ship state is incomplete; the write was refused");
            return None;
        }
        let path = self.sessions_dir.join(format!("{project_id}.json"));
        match crate::infra::persistence::locked_merge_write_stamped(&path, &state, merge_states) {
            Ok(written) => Some(written),
            Err(e) => {
                tracing::warn!("ship: failed to persist state: {e}");
                None
            }
        }
    }

    pub fn clear(&self, project_id: &str) {
        if !self.enabled {
            return;
        }
        let path = self.sessions_dir.join(format!("{project_id}.json"));
        let _ = self.locked(project_id, || match fs::remove_file(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        });
    }
}
