use std::env;
use std::fs;
use std::path::PathBuf;

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource, WriteError};
use serde::{Deserialize, Serialize};

use super::saved_cycle::SavedCycle;
use crate::infra::read_json;
use crate::run::application::RunError;
use crate::run::domain::execution_identity::ExecutionIdentity;
use crate::run::domain::goal::Goal;
use crate::run::domain::step::Step;

#[path = "persistence_upgrade.rs"]
mod upgrade;

const SCHEMA_VERSION: u32 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedState {
    pub(crate) schema_version: u32,
    pub(crate) project_id: String,
    pub(crate) goal: Option<Goal>,
    pub(crate) steps: Vec<Step>,
    pub(crate) next_progress_id: u32,
    pub(crate) execution: ExecutionIdentity,
    pub(crate) selection_revision: u64,
    pub(crate) saved_cycles: Vec<SavedCycle>,
}

impl PersistedState {
    pub(crate) fn into_observation(self) -> magistr_workflow_contracts::RunData {
        use magistr_workflow_contracts::{Run, RunData};
        RunData {
            current: self.goal.map(|goal| Run {
                run_id: self.execution.run_id,
                step_instances: self.execution.steps,
                goal,
                steps: self.steps,
            }),
            retained: self
                .saved_cycles
                .into_iter()
                .map(|cycle| Run {
                    run_id: cycle.execution.run_id,
                    step_instances: cycle.execution.steps,
                    goal: cycle.goal,
                    steps: cycle.steps,
                })
                .collect(),
        }
    }

    pub(crate) fn empty(project_id: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project_id,
            goal: None,
            steps: Vec::new(),
            next_progress_id: 1,
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
                .goal
                .as_ref()
                .is_some_and(|o| o.project_id != project_id)
            || self
                .saved_cycles
                .iter()
                .any(|c| c.goal.project_id != project_id)
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
        if !self.execution.has_step_instances(&self.steps)
            || self
                .saved_cycles
                .iter()
                .any(|c| !c.execution.has_step_instances(&c.steps))
        {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The saved work contains incomplete or duplicate steps.",
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
        let enabled = env::var("MAGISTR_WORKFLOW_PERSIST")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        let data_dir = env::var("MAGISTR_WORKFLOW_DATA_DIR")
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
    /// The saved format this family writes. A document positively older is
    /// converted by `upgrade_document` before any reader opens it.
    pub const CURRENT_VERSION: u32 = SCHEMA_VERSION;

    /// Validate a saved execution before its storage owner relocates it.
    pub fn validate_document(bytes: &[u8], project_id: &str) -> Result<(), ReadError> {
        let document: PersistedState = serde_json::from_slice(bytes)
            .map_err(|error| ReadError::new(ReadErrorKind::Schema, error.to_string()))?;
        document.validate(project_id)
    }

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
        operation: impl FnOnce() -> Result<R, RunError>,
    ) -> Result<R, RunError> {
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
        // Partition under `run/` so the decision family writes to its own
        // sibling subdirectory and the two never share a `<project>.json`
        // path. Mirrors the layout used by `crate::infra::Persistence`.
        let sessions_dir = cfg.data_dir.join("run").join("sessions");
        if cfg.enabled
            && let Err(e) = fs::create_dir_all(&sessions_dir)
        {
            tracing::warn!(
                "run: could not create data dir {}: {e}",
                sessions_dir.display()
            );
        }
        Self {
            enabled: cfg.enabled,
            sessions_dir,
        }
    }

    /// Each execution owns its selected cycle and check_result ledger within the project store.
    #[must_use]
    pub fn for_execution(mut self, key: &str) -> Self {
        let run = self
            .sessions_dir
            .parent()
            .expect("run sessions have a parent");
        self.sessions_dir = run.join("executions").join(key).join("sessions");
        self
    }

    /// The file selected by this persistence owner for one project's execution.
    pub fn path_for(&self, project_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{project_id}.json"))
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The check_result ledger beside this store's sessions: on disk under
    /// `run/check_results/<project>` when persistence is on, in memory otherwise.
    pub(crate) fn check_result_ledger(
        &self,
        project_id: &str,
    ) -> crate::run::check_result_ledger::CheckResultLedger {
        if !self.enabled {
            return crate::run::check_result_ledger::CheckResultLedger::in_memory();
        }
        let root = self
            .sessions_dir
            .parent()
            .map_or_else(|| self.sessions_dir.clone(), std::path::Path::to_path_buf);
        crate::run::check_result_ledger::CheckResultLedger::on_disk(
            root.join("check_results").join(project_id),
        )
    }

    /// Pin the selected run for the complete prune; ledger lock is always second.
    pub(crate) fn compact_check_results(
        &self,
        project_id: &str,
        expected_run: &str,
        retained: impl FnOnce() -> Result<std::collections::BTreeSet<String>, String>,
    ) -> Result<crate::run::check_result_compaction::CheckResultCompaction, RunError> {
        self.locked(project_id, || {
            let state = self
                .current_stamped(project_id)?
                .value
                .filter(|s| s.goal.is_some())
                .ok_or("Open a run before clearing old check history.")?;
            if state.execution.run_id != expected_run {
                return Err(RunError::refused(
                    "stale_target",
                    "The selected work changed. Read it again before clearing check history.",
                ));
            }
            let ledger = self.check_result_ledger(project_id);
            let saved_check_results: std::collections::BTreeSet<_> = state
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
                ids.extend(saved_check_results);
                Ok(ids)
            })
        })
    }
}
