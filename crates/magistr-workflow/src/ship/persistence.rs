use std::env;
use std::fs;
use std::path::PathBuf;

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource, WriteError};
use serde::{Deserialize, Serialize};

use super::saved_cycle::SavedCycle;
use crate::infra::read_json;
use crate::ship::application::ShipError;
use crate::ship::domain::execution_identity::ExecutionIdentity;
use crate::ship::domain::objective::Objective;
use crate::ship::domain::task::Task;

const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    pub(crate) schema_version: u32,
    pub(crate) project_id: String,
    pub(crate) objective: Option<Objective>,
    pub(crate) tasks: Vec<Task>,
    pub(crate) next_action_id: u32,
    pub(crate) execution: ExecutionIdentity,
    pub(crate) selection_revision: u64,
    pub(crate) saved_cycles: Vec<SavedCycle>,
}

impl PersistedState {
    pub(crate) fn into_observation(self) -> magistr_workflow_contracts::ShipData {
        use magistr_workflow_contracts::{ShipData, ShipRun};
        ShipData {
            current: self.objective.map(|objective| ShipRun {
                run_id: self.execution.run_id,
                task_instances: self.execution.tasks,
                objective,
                tasks: self.tasks,
            }),
            retained: self
                .saved_cycles
                .into_iter()
                .map(|cycle| ShipRun {
                    run_id: cycle.execution.run_id,
                    task_instances: cycle.execution.tasks,
                    objective: cycle.objective,
                    tasks: cycle.tasks,
                })
                .collect(),
        }
    }

    pub(crate) fn empty(project_id: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project_id,
            objective: None,
            tasks: Vec::new(),
            next_action_id: 1,
            execution: ExecutionIdentity::fresh(),
            selection_revision: 0,
            saved_cycles: Vec::new(),
        }
    }

    pub(crate) fn validate(&self, project_id: &str) -> Result<(), ReadError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ReadError::new(
                ReadErrorKind::Schema,
                "The saved work uses an unsupported format.",
            ));
        }
        if self.project_id != project_id
            || self
                .objective
                .as_ref()
                .is_some_and(|o| o.project_id != project_id)
            || self
                .saved_cycles
                .iter()
                .any(|c| c.objective.project_id != project_id)
        {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The saved work could not be matched to this project.",
            ));
        }
        let runs = std::iter::once(&self.execution)
            .chain(self.saved_cycles.iter().map(|cycle| &cycle.execution));
        let mut identities = std::collections::BTreeSet::new();
        if !runs.into_iter().all(|run| identities.insert(&run.run_id)) {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The saved work contains duplicate work records.",
            ));
        }
        if !self.execution.has_task_instances(&self.tasks)
            || self
                .saved_cycles
                .iter()
                .any(|c| !c.execution.has_task_instances(&c.tasks))
        {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The saved work contains incomplete or duplicate tasks.",
            ));
        }
        Ok(())
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
        let read: Observed<Option<PersistedState>> = read_json(&self.path_for(project_id))?;
        if let Some(state) = &read.value {
            state.validate(project_id)?;
        }
        Ok(read)
    }

    pub(crate) fn lock_path(&self, project_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{project_id}.json.lock"))
    }

    pub(crate) fn locked<R>(
        &self,
        project_id: &str,
        operation: impl FnOnce() -> Result<R, ShipError>,
    ) -> Result<R, ShipError> {
        fs::create_dir_all(&self.sessions_dir).map_err(|e| {
            WriteError::before_commit(format!("The saved work could not be opened: {e}"))
        })?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.lock_path(project_id))
            .map_err(|e| {
                WriteError::before_commit(format!("The saved work could not be locked: {e}"))
            })?;
        lock.lock().map_err(|e| {
            WriteError::before_commit(format!("The saved work could not be locked: {e}"))
        })?;
        operation()
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

    /// Pin the selected run for the complete prune; ledger lock is always second.
    pub(crate) fn compact_receipts(
        &self,
        project_id: &str,
        expected_run: &str,
        retained: impl FnOnce() -> Result<std::collections::BTreeSet<String>, String>,
    ) -> Result<crate::ship::receipt_compaction::ReceiptCompaction, ShipError> {
        self.locked(project_id, || {
            let state = self
                .current_stamped(project_id)?
                .value
                .filter(|s| s.objective.is_some())
                .ok_or("Set an objective before clearing old check history.")?;
            if state.execution.run_id != expected_run {
                return Err(ShipError::refused(
                    "stale_target",
                    "The selected work changed. Read it again before clearing check history.",
                ));
            }
            let ledger = self.receipt_ledger(project_id);
            let saved_receipts: std::collections::BTreeSet<_> = state
                .saved_cycles
                .iter()
                .map(|cycle| ledger.for_run(&cycle.execution.run_id))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .map(|r| r.id)
                .collect();
            ledger.compact(&state.execution.run_id, || {
                let mut ids = retained()?;
                ids.extend(saved_receipts);
                Ok(ids)
            })
        })
    }
}
