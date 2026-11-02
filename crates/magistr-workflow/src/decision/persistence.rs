//! One project document owns decision logs, identities, and numbering.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};
use serde::{Deserialize, Serialize};

use crate::decision::domain::Log;
use crate::infra::transaction::{Committed, TransactionError, TransactionTarget, transact};
use crate::infra::{PersistenceConfig, read_json};

#[path = "persistence_upgrade.rs"]
mod upgrade;

const SCHEMA_VERSION: u32 = 6;

/// The complete transaction boundary. Selected logs are projections of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PersistedProject {
    pub schema_version: u32,
    pub project_id: String,
    pub logs: BTreeMap<String, Log>,
}

impl PersistedProject {
    pub fn empty(project_id: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project_id: project_id.to_owned(),
            logs: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Persistence {
    enabled: bool,
    documents_dir: PathBuf,
    project_id: String,
}

impl Persistence {
    /// The saved format this family writes. A document positively older is
    /// converted by `upgrade_document` before any reader opens it.
    pub const CURRENT_VERSION: u32 = SCHEMA_VERSION;

    pub fn for_project(cfg: &PersistenceConfig, project_id: &str) -> Self {
        Self {
            enabled: cfg.enabled,
            documents_dir: cfg.data_dir.join("decisions").join("sessions"),
            project_id: project_id.to_owned(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn documents_dir(&self) -> &Path {
        &self.documents_dir
    }

    pub(crate) fn load_project(&self) -> Result<Observed<Option<PersistedProject>>, ReadError> {
        if !self.enabled {
            return Ok(Observed {
                value: None,
                source: ReadSource::Memory,
            });
        }
        let read: Observed<Option<PersistedProject>> = read_json(&self.store_path()?)?;
        if let Some(project) = &read.value {
            self.validate(project)?;
        }
        Ok(read)
    }

    pub fn load_default(&self) -> Result<Observed<Option<Log>>, ReadError> {
        self.load_log(&self.project_id)
    }

    /// The source describes the project document even when this history is absent.
    pub fn load_log(&self, log_id: &str) -> Result<Observed<Option<Log>>, ReadError> {
        self.validate_log_id(log_id)?;
        Ok(self
            .load_project()?
            .map(|project| project.and_then(|mut project| project.logs.remove(log_id))))
    }

    pub fn load_logs(&self) -> Result<HashMap<String, Log>, ReadError> {
        Ok(self
            .load_project()?
            .value
            .map_or_else(HashMap::new, |project| {
                project
                    .logs
                    .into_iter()
                    .filter(|(id, _)| id != &self.project_id)
                    .collect()
            }))
    }

    pub(crate) fn transact<R, E>(
        &self,
        memory: &PersistedProject,
        apply: impl FnOnce(&mut PersistedProject) -> Result<R, E>,
    ) -> Result<Committed<PersistedProject, R>, TransactionError<E>> {
        if !self.enabled {
            return transact(
                TransactionTarget::Memory(memory),
                || PersistedProject::empty(&self.project_id),
                |project| self.validate(project),
                apply,
            );
        }
        let path = self.store_path().map_err(|error| {
            TransactionError::Store(magistr_workflow_contracts::WriteError::read(error))
        })?;
        let lock_path = path.with_extension("json.lock");
        transact(
            TransactionTarget::File {
                path: &path,
                lock_path: &lock_path,
            },
            || PersistedProject::empty(&self.project_id),
            |project| self.validate(project),
            apply,
        )
    }

    pub fn store_path(&self) -> Result<PathBuf, ReadError> {
        if !is_safe_id(&self.project_id) {
            return Err(identity(
                "The project could not be matched to a saved reasoning file.",
            ));
        }
        Ok(self.documents_dir.join(format!("{}.json", self.project_id)))
    }

    pub(crate) fn validate_log_id(&self, log_id: &str) -> Result<(), ReadError> {
        let prefix = format!("{}{}", self.project_id, crate::infra::PROJECT_SEP);
        if !is_safe_id(log_id) || (log_id != self.project_id && !log_id.starts_with(&prefix)) {
            return Err(identity(
                "The reasoning log could not be matched to this project.",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate(&self, project: &PersistedProject) -> Result<(), ReadError> {
        if project.schema_version != SCHEMA_VERSION {
            return Err(ReadError::new(
                ReadErrorKind::Schema,
                "The saved reasoning uses an unsupported format.",
            ));
        }
        if project.project_id != self.project_id {
            return Err(identity(
                "The saved reasoning could not be matched to this project.",
            ));
        }
        let mut numbers = HashSet::new();
        let mut ids = HashSet::new();
        for (log_id, history) in &project.logs {
            self.validate_log_id(log_id)?;
            let expected = (log_id != &self.project_id).then_some(log_id.as_str());
            if history.log_id.as_deref() != expected
                || history
                    .metadata
                    .as_ref()
                    .and_then(|meta| meta.project_id.as_deref())
                    != Some(self.project_id.as_str())
            {
                return Err(identity(
                    "The saved reasoning could not be matched to its project and log.",
                ));
            }
            let mut thread_ids = HashSet::new();
            if history
                .threads
                .iter()
                .flatten()
                .any(|thread| thread.id.trim().is_empty() || !thread_ids.insert(thread.id.as_str()))
            {
                return Err(identity(
                    "The saved reasoning contains incomplete or duplicate questions.",
                ));
            }
            for note in &history.notes {
                let Some(id) = note.record_id.as_deref().filter(|id| !id.trim().is_empty()) else {
                    return Err(identity(
                        "The saved reasoning contains an item without an identity.",
                    ));
                };
                if note.note == 0 || !numbers.insert(note.note) || !ids.insert(id) {
                    return Err(identity(
                        "The saved reasoning contains duplicate item identities or numbers.",
                    ));
                }
                if note
                    .thread_id
                    .as_deref()
                    .is_some_and(|id| !thread_ids.contains(id))
                {
                    return Err(identity(
                        "A saved reasoning item refers to an unknown question.",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn identity(message: &str) -> ReadError {
    ReadError::new(ReadErrorKind::Identity, message)
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && id != "."
        && id != ".."
}

#[cfg(test)]
#[path = "persistence.tests.rs"]
mod tests;
