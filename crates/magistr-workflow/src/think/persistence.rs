//! One project document owns reasoning histories, identities, and numbering.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};
use serde::{Deserialize, Serialize};

use crate::infra::transaction::{Committed, TransactionError, TransactionTarget, transact};
use crate::infra::{PersistenceConfig, read_json};
use crate::think::domain::ThinkHistory;

const SCHEMA_VERSION: u32 = 2;

/// The complete transaction boundary. Selected histories are projections of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PersistedProject {
    pub schema_version: u32,
    pub project_id: String,
    pub histories: BTreeMap<String, ThinkHistory>,
}

impl PersistedProject {
    pub fn empty(project_id: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project_id: project_id.to_owned(),
            histories: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Persistence {
    enabled: bool,
    sessions_dir: PathBuf,
    project_id: String,
}

impl Persistence {
    pub fn for_project(cfg: &PersistenceConfig, project_id: &str) -> Self {
        Self {
            enabled: cfg.enabled,
            sessions_dir: cfg.data_dir.join("think").join("sessions"),
            project_id: project_id.to_owned(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
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

    pub fn load_default(&self) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        self.load_session(&self.project_id)
    }

    /// The source describes the project document even when this history is absent.
    pub fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        self.validate_session_id(session_id)?;
        Ok(self
            .load_project()?
            .map(|project| project.and_then(|mut project| project.histories.remove(session_id))))
    }

    pub fn load_sessions(&self) -> Result<HashMap<String, ThinkHistory>, ReadError> {
        Ok(self
            .load_project()?
            .value
            .map_or_else(HashMap::new, |project| {
                project
                    .histories
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
        Ok(self.sessions_dir.join(format!("{}.json", self.project_id)))
    }

    pub(crate) fn validate_session_id(&self, session_id: &str) -> Result<(), ReadError> {
        let prefix = format!("{}{}", self.project_id, crate::infra::PROJECT_SEP);
        if !is_safe_id(session_id)
            || (session_id != self.project_id && !session_id.starts_with(&prefix))
        {
            return Err(identity(
                "The reasoning session could not be matched to this project.",
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
        for (session_id, history) in &project.histories {
            self.validate_session_id(session_id)?;
            let expected = (session_id != &self.project_id).then_some(session_id.as_str());
            if history.session_id.as_deref() != expected
                || history
                    .metadata
                    .as_ref()
                    .and_then(|meta| meta.project_id.as_deref())
                    != Some(self.project_id.as_str())
            {
                return Err(identity(
                    "The saved reasoning could not be matched to its project and session.",
                ));
            }
            let mut branch_ids = HashSet::new();
            if history
                .branches
                .iter()
                .flatten()
                .any(|branch| branch.id.trim().is_empty() || !branch_ids.insert(branch.id.as_str()))
            {
                return Err(identity(
                    "The saved reasoning contains incomplete or duplicate questions.",
                ));
            }
            for step in &history.steps {
                let Some(id) = step.record_id.as_deref().filter(|id| !id.trim().is_empty()) else {
                    return Err(identity(
                        "The saved reasoning contains an item without an identity.",
                    ));
                };
                if step.step_number == 0 || !numbers.insert(step.step_number) || !ids.insert(id) {
                    return Err(identity(
                        "The saved reasoning contains duplicate item identities or numbers.",
                    ));
                }
                if step
                    .branch_id
                    .as_deref()
                    .is_some_and(|id| !branch_ids.contains(id))
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
