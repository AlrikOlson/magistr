use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::infra::{MergedWrite, StoreStamp};
use crate::ship::domain::execution_identity::ExecutionIdentity;
use crate::ship::domain::objective::Objective;
use crate::ship::domain::task::Task;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    schema_version: u32,
    project_id: String,
    pub(crate) objective: Option<Objective>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) next_action_id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) execution: Option<ExecutionIdentity>,
}

impl PersistedState {
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
/// - **Different cycles**: the newer cycle wins outright. A stale process
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
    if mem_stamp == disk_stamp {
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
    let disk_is_newer = match (disk_stamp, mem_stamp) {
        (Some(d), Some(m)) => newer(d, m),
        (Some(_), None) => true,
        _ => false,
    };
    if disk_is_newer {
        disk
    } else {
        PersistedState {
            schema_version: memory.schema_version,
            project_id: memory.project_id.clone(),
            objective: memory.objective.clone(),
            tasks: memory.tasks.to_vec(),
            next_action_id: memory.next_action_id,
            execution: memory.execution.clone(),
        }
    }
}

fn merge_same_cycle(memory: &PersistedState, disk: PersistedState) -> PersistedState {
    let memory_identity = memory.execution.clone().unwrap_or_else(|| {
        ExecutionIdentity::legacy(&memory.project_id, &memory.objective, &memory.tasks)
    });
    let disk_identity = disk.execution.unwrap_or_else(|| {
        ExecutionIdentity::legacy(&disk.project_id, &disk.objective, &disk.tasks)
    });
    let mut identity = memory_identity.clone();
    identity
        .removed_instances
        .extend(disk_identity.removed_instances.iter().cloned());
    identity.tasks.clear();
    let mut tasks: Vec<Task> = Vec::new();
    for (source, identities) in [
        (&memory.tasks, &memory_identity),
        (&disk.tasks, &disk_identity),
    ] {
        for task in source {
            let instance = identities
                .tasks
                .get(&task.id)
                .cloned()
                .unwrap_or_else(|| format!("legacy:task:{}", task.id));
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
        execution: Some(identity),
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
    /// The persisted cycle with the stamp of the file it was read from,
    /// taken before the read so a write landing in between costs one
    /// spurious re-read later and never a missed one.
    pub(crate) fn current_stamped(
        &self,
        project_id: &str,
    ) -> std::io::Result<(Option<PersistedState>, Option<StoreStamp>)> {
        let path = self.sessions_dir.join(format!("{project_id}.json"));
        let stamp = StoreStamp::of(&path)?;
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((None, None)),
            Err(error) => return Err(error),
        };
        let state: PersistedState =
            serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        if state.schema_version != SCHEMA_VERSION || state.project_id != project_id {
            return Err(std::io::Error::other(
                "Ship state schema or project identity does not match",
            ));
        }
        Ok((Some(state), stamp))
    }

    /// The stamp of the persisted cycle as it stands: `None` when
    /// persistence is off or there is no file. A `stat`, never a read.
    pub(crate) fn stamp(&self, project_id: &str) -> std::io::Result<Option<StoreStamp>> {
        if !self.enabled {
            return Ok(None);
        }
        StoreStamp::of(&self.sessions_dir.join(format!("{project_id}.json")))
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
            let (current, stamp) = self.current_stamped(&target.project_id)?;
            let Some(mut state) = current else {
                return Ok((None, false, None));
            };
            let identity = state.execution.clone().unwrap_or_else(|| {
                ExecutionIdentity::legacy(&target.project_id, &state.objective, &state.tasks)
            });
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
            state.execution = Some(identity);
            let path = self
                .sessions_dir
                .join(format!("{}.json", target.project_id));
            let temporary = path.with_extension("json.tmp");
            fs::write(
                &temporary,
                serde_json::to_vec(&state).map_err(std::io::Error::other)?,
            )?;
            fs::rename(temporary, &path)?;
            let stamp = StoreStamp::of(&path).ok().flatten();
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
            let (current, _) = self.current_stamped(project_id)?;
            let state = current.filter(|s| s.objective.is_some()).ok_or_else(|| {
                std::io::Error::other("Set an objective before clearing old check history.")
            })?;
            let identity = state.execution.unwrap_or_else(|| {
                ExecutionIdentity::legacy(project_id, &state.objective, &state.tasks)
            });
            self.receipt_ledger(project_id)
                .compact(&identity.run_id, retained)
                .map_err(std::io::Error::other)
        })
    }

    /// Persist the cycle under the locked merge-on-save discipline (see
    /// `merge_states`) so a concurrent process's acked tasks — or its newer
    /// cycle — are never erased by a stale writer.
    pub fn save(
        &self,
        project_id: &str,
        objective: &Option<Objective>,
        tasks: &[Task],
        next_action_id: u32,
    ) {
        self.save_state(project_id, objective, tasks, next_action_id, None);
    }

    /// [`Self::save`] with the run identity, answering what landed on disk
    /// and the stamp of the written file; `None` when persistence is off
    /// or the write failed (logged).
    pub(crate) fn save_with_identity(
        &self,
        project_id: &str,
        objective: &Option<Objective>,
        tasks: &[Task],
        next_action_id: u32,
        execution: &ExecutionIdentity,
    ) -> Option<MergedWrite<PersistedState>> {
        self.save_state(
            project_id,
            objective,
            tasks,
            next_action_id,
            Some(execution.clone()),
        )
    }

    fn save_state(
        &self,
        project_id: &str,
        objective: &Option<Objective>,
        tasks: &[Task],
        next_action_id: u32,
        execution: Option<ExecutionIdentity>,
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
        };
        let path = self.sessions_dir.join(format!("{project_id}.json"));
        match crate::infra::persistence::locked_merge_write_stamped(&path, &state, merge_states) {
            Ok(written) => Some(written),
            Err(e) => {
                tracing::warn!("ship: failed to persist state: {e}");
                None
            }
        }
    }

    pub fn load(&self, project_id: &str) -> Option<(Option<Objective>, Vec<Task>, u32)> {
        self.load_with_identity(project_id)
            .map(|(objective, tasks, next_id, _)| (objective, tasks, next_id))
    }

    pub(crate) fn load_with_identity(
        &self,
        project_id: &str,
    ) -> Option<(Option<Objective>, Vec<Task>, u32, ExecutionIdentity)> {
        if !self.enabled {
            return None;
        }
        let path = self.sessions_dir.join(format!("{project_id}.json"));
        let data = fs::read_to_string(&path).ok()?;
        let state: PersistedState = serde_json::from_str(&data).ok()?;
        if state.schema_version != SCHEMA_VERSION {
            tracing::warn!(
                "ship: skipping {} (schema v{}, expected v{SCHEMA_VERSION})",
                path.display(),
                state.schema_version
            );
            return None;
        }
        let execution = state.execution.unwrap_or_else(|| {
            ExecutionIdentity::legacy(project_id, &state.objective, &state.tasks)
        });
        Some((
            state.objective,
            state.tasks,
            state.next_action_id,
            execution,
        ))
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
