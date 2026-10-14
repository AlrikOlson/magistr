//! Project-scoped reasoning archives with validated reads and merge-on-save writes.

use std::collections::{HashMap, HashSet};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use magistr_workflow_contracts::{Observed, ReadError, ReadErrorKind, ReadSource};
use serde::{Deserialize, Serialize};

use crate::infra::{PersistenceConfig, read_json};
use crate::think::domain::ThinkHistory;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedHistory {
    schema_version: u32,
    history: ThinkHistory,
}

#[derive(Debug, Clone)]
pub struct Persistence {
    enabled: bool,
    sessions_dir: PathBuf,
    stem: String,
}

impl Persistence {
    /// Select this project's archive without opening another project's history.
    pub fn for_project(cfg: &PersistenceConfig, project_id: &str) -> Self {
        Self {
            enabled: cfg.enabled,
            sessions_dir: cfg.data_dir.join("think").join("sessions"),
            stem: project_id.to_owned(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// Observe the project archive, preserving absence and read refusals.
    pub fn load_default(&self) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        if !self.enabled {
            return Ok(Observed {
                value: None,
                source: ReadSource::Memory,
            });
        }
        self.read_project_history(&self.default_store_path()?, None)
    }

    /// Observe one named session through this project's selected persistence owner.
    pub fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        if session_id == self.stem {
            return self.load_default();
        }
        let path = self.session_store_path(session_id)?;
        if !self.enabled {
            return Ok(Observed {
                value: None,
                source: ReadSource::Memory,
            });
        }
        self.read_project_history(&path, Some(session_id))
    }

    fn read_project_history(
        &self,
        path: &Path,
        session_id: Option<&str>,
    ) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
        let read = read_history(path)?;
        if let Some(history) = &read.value {
            let owner = history
                .metadata
                .as_ref()
                .and_then(|meta| meta.project_id.as_deref());
            if owner.is_some_and(|id| id != self.stem)
                || (!history.steps.is_empty() && owner != Some(self.stem.as_str()))
            {
                return Err(ReadError::new(
                    ReadErrorKind::Identity,
                    "The saved reasoning could not be matched to this project.",
                ));
            }
            if history.session_id.as_deref() != session_id {
                tracing::warn!(expected = ?session_id, stored = ?history.session_id, "Reasoning session does not match its selected file");
                return Err(ReadError::new(
                    ReadErrorKind::Identity,
                    "The saved reasoning could not be matched to this session.",
                ));
            }
        }
        Ok(read)
    }

    /// Load this project's explicitly named sessions; a refused file is an error.
    pub fn load_sessions(&self) -> Result<HashMap<String, ThinkHistory>, ReadError> {
        let mut out = HashMap::new();
        if !self.enabled {
            return Ok(out);
        }
        self.default_store_path()?;
        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(out),
            Err(error) => return Err(read_error(error)),
        };
        let prefix = format!("{}{}", self.stem, crate::infra::PROJECT_SEP);
        for entry in entries {
            let path = entry.map_err(read_error)?.path();
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !stem.starts_with(&prefix)
                || path.extension().and_then(|s| s.to_str()) != Some("json")
            {
                continue;
            }
            if let Some(history) = self.load_session(stem)?.value {
                out.insert(stem.to_owned(), history);
            }
        }
        Ok(out)
    }

    pub fn save_default(&self, history: &ThinkHistory) {
        if !self.enabled {
            return;
        }
        let path = match self.default_store_path() {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(%error, "Reasoning could not be saved");
                return;
            }
        };
        if let Err(error) = locked_write(&path, history) {
            tracing::warn!(%error, "Reasoning could not be saved");
        }
    }

    pub fn save_session(&self, session_id: &str, history: &ThinkHistory) {
        if !self.enabled {
            return;
        }
        let path = match self.session_store_path(session_id) {
            Ok(path) => path,
            Err(error) => {
                tracing::warn!(%error, "Reasoning session could not be saved");
                return;
            }
        };
        if let Err(error) = locked_write(&path, history) {
            tracing::warn!(%error, "Reasoning session could not be saved");
        }
    }

    /// Remove this project's persisted files: the default history plus every
    /// session namespaced under it. Called by `clear_history` so the disk
    /// state matches the in-memory wipe — scoped to the project so a wipe
    /// can never destroy other projects' traces in the shared data dir.
    pub fn delete_project_files(&self) {
        if !self.enabled {
            return;
        }
        let session_prefix = format!("{}{}", self.stem, crate::infra::PROJECT_SEP);
        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => {
                tracing::warn!(
                    "Failed to read sessions dir {}: {e}",
                    self.sessions_dir.display()
                );
                return;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            // Project ids may legitimately contain dots, so strip the known
            // artifact suffixes rather than splitting on the first dot.
            let Some(stem_of) = name
                .strip_suffix(".json")
                .or_else(|| name.strip_suffix(".json.lock"))
                .or_else(|| name.strip_suffix(".json.tmp"))
            else {
                continue;
            };
            if stem_of != self.stem && !stem_of.starts_with(&session_prefix) {
                continue;
            }
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to delete {}: {e}", path.display());
            }
        }
    }

    fn path_for(&self, stem: &str) -> PathBuf {
        self.sessions_dir.join(format!("{stem}.json"))
    }

    /// Select a session file only within this project's namespace.
    pub fn session_store_path(&self, session_id: &str) -> Result<PathBuf, ReadError> {
        let default = self.default_store_path()?;
        if session_id == self.stem {
            return Ok(default);
        }
        let prefix = format!("{}{}", self.stem, crate::infra::PROJECT_SEP);
        if !is_safe_session_id(session_id) || !session_id.starts_with(&prefix) {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The reasoning session could not be matched to this project.",
            ));
        }
        Ok(self.path_for(session_id))
    }

    /// The selected archive path; invalid project identity is refused.
    pub fn default_store_path(&self) -> Result<PathBuf, ReadError> {
        if !is_safe_session_id(&self.stem) {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The project could not be matched to a saved reasoning file.",
            ));
        }
        Ok(self.path_for(&self.stem))
    }
}

fn read_history(path: &Path) -> Result<Observed<Option<ThinkHistory>>, ReadError> {
    let read: Observed<Option<PersistedHistory>> = read_json(path)?;
    let value = match read.value {
        None => None,
        Some(saved) => {
            if saved.schema_version != SCHEMA_VERSION {
                return Err(ReadError::new(
                    ReadErrorKind::Schema,
                    "The saved reasoning uses an unsupported format.",
                ));
            }
            validate_history(&saved.history)?;
            Some(saved.history)
        }
    };
    Ok(Observed {
        value,
        source: read.source,
    })
}

fn validate_history(history: &ThinkHistory) -> Result<(), ReadError> {
    let mut numbers = HashSet::new();
    let mut ids = HashSet::new();
    for step in &history.steps {
        let Some(id) = step.record_id.as_deref().filter(|id| !id.trim().is_empty()) else {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The saved reasoning contains an incomplete item.",
            ));
        };
        if step.step_number == 0 || !numbers.insert(step.step_number) || !ids.insert(id) {
            return Err(ReadError::new(
                ReadErrorKind::Identity,
                "The saved reasoning contains incomplete or duplicate items.",
            ));
        }
    }
    let mut branches = HashSet::new();
    if history
        .branches
        .iter()
        .flatten()
        .any(|branch| branch.id.trim().is_empty() || !branches.insert(&branch.id))
    {
        return Err(ReadError::new(
            ReadErrorKind::Identity,
            "The saved reasoning contains incomplete or duplicate branches.",
        ));
    }
    Ok(())
}

fn read_error(error: io::Error) -> ReadError {
    tracing::warn!(%error, "Reasoning sessions could not be read");
    ReadError::new(
        ReadErrorKind::Io,
        "The saved reasoning could not be read. Restore access and try again.",
    )
}

fn locked_write(path: &Path, history: &ThinkHistory) -> io::Result<()> {
    let state = PersistedHistory {
        schema_version: SCHEMA_VERSION,
        history: history.clone(),
    };
    crate::infra::persistence::locked_merge_write(path, &state, |memory, disk| {
        if disk.schema_version != SCHEMA_VERSION {
            return memory.clone();
        }
        PersistedHistory {
            schema_version: SCHEMA_VERSION,
            history: merge_histories(&memory.history, disk.history),
        }
    })
}

/// Union stored step and branch identities; existing records keep their authored identity.
pub(crate) fn merge_histories(memory: &ThinkHistory, disk: ThinkHistory) -> ThinkHistory {
    let mut merged = memory.clone();
    let known: HashSet<u32> = merged.steps.iter().map(|s| s.step_number).collect();
    let known_content: HashSet<(String, String, String)> = merged
        .steps
        .iter()
        .filter_map(|s| s.content_key())
        .collect();
    let known_ids: HashSet<&str> = merged
        .steps
        .iter()
        .filter_map(|s| s.record_id.as_deref())
        .collect();
    let mut adopted: Vec<_> = disk
        .steps
        .into_iter()
        .filter(|s| {
            !known.contains(&s.step_number)
                && s.record_id
                    .as_deref()
                    .is_none_or(|id| !known_ids.contains(id))
                && s.content_key().is_none_or(|k| !known_content.contains(&k))
        })
        .collect();
    let mem_branch_ids: HashSet<String> = merged
        .branches
        .iter()
        .flatten()
        .map(|b| b.id.clone())
        .collect();
    let extra_branches: Vec<_> = disk
        .branches
        .into_iter()
        .flatten()
        .filter(|b| !mem_branch_ids.contains(&b.id))
        .collect();
    if adopted.is_empty() && extra_branches.is_empty() {
        return merged;
    }
    if !adopted.is_empty() {
        merged.steps.append(&mut adopted);
        merged.steps.sort_by_key(|s| s.step_number);
    }
    if !extra_branches.is_empty() {
        merged
            .branches
            .get_or_insert_with(Vec::new)
            .extend(extra_branches);
    }
    merged
}

/// Strict allowlist for session-id filenames. Prevents path traversal
/// (`..`), separators (`/`, `\`), and weird shell characters from landing
/// on disk regardless of what the LLM sends.
fn is_safe_session_id(id: &str) -> bool {
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
