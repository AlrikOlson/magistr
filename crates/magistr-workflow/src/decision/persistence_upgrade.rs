//! Consolidate older per-session histories into the current project document,
//! and move saved decision logs to the glossary's words.
use super::{
    Log, PersistedProject, Persistence, ReadError, ReadErrorKind, SCHEMA_VERSION, identity,
};
use crate::infra::StoredRefWords;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

impl Persistence {
    /// Convert one project's saved decision log, as every family codec does:
    /// a current document comes back unchanged.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let owner = Self::for_project(
            &crate::infra::PersistenceConfig {
                enabled: false,
                data_dir: std::path::PathBuf::new(),
            },
            project_id,
        );
        owner.upgrade_documents(&BTreeMap::from([(project_id.to_owned(), bytes.to_vec())]))
    }

    /// Convert recognized history files under their filename identities.
    /// Admission owns locking and publication; this conversion has no filesystem effects.
    pub fn upgrade_documents(
        &self,
        documents: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Vec<u8>, ReadError> {
        let mut project =
            serde_json::to_value(PersistedProject::empty(&self.project_id)).map_err(schema)?;
        let mut legacy = Vec::new();
        let mut current = None;
        for (session, bytes) in documents {
            self.validate_log_id(session)?;
            let value: Value = serde_json::from_slice(bytes).map_err(schema)?;
            match value["schema_version"].as_u64() {
                Some(1) => legacy.push((session, value)),
                Some(version @ (2..=6)) if session == &self.project_id => {
                    // Version 2 still spells run references `task:` and
                    // `action:`; versions 2 and 3 spell plan tasks `chunk:`;
                    // versions 2 to 4 spell notes `think:` and keep the
                    // decision log's older keys; versions 2 to 5 spell
                    // requests `signal:`.
                    let mut value = value;
                    let words = match version {
                        2 => Some(StoredRefWords::BeforeRunWords),
                        3 => Some(StoredRefWords::BeforePlanWords),
                        4 => Some(StoredRefWords::BeforeDecisionWords),
                        5 => Some(StoredRefWords::BeforeRequestWords),
                        _ => None,
                    };
                    if let Some(words) = words {
                        crate::infra::CrossRef::upgrade_stored_refs(&mut value, words);
                        if version < 5 {
                            rename_project_words(&mut value);
                        }
                        value["schema_version"] = json!(SCHEMA_VERSION);
                    }
                    let saved: PersistedProject =
                        serde_json::from_value(value.clone()).map_err(schema)?;
                    self.validate(&saved)?;
                    current = (version == u64::from(SCHEMA_VERSION)).then_some(bytes);
                    project = value;
                }
                _ => {
                    return Err(schema(
                        "The saved reasoning uses an unsupported format; the original is unchanged.",
                    ));
                }
            }
        }
        if legacy.is_empty() {
            if let Some(current) = current {
                return Ok(current.clone());
            }
            if project["logs"].as_object().is_some_and(|h| !h.is_empty()) {
                return crate::infra::versioned_document(project, SCHEMA_VERSION).map_err(schema);
            }
            return Err(schema("No saved reasoning was selected."));
        }
        for (session, value) in legacy {
            let mut history = value["history"].clone();
            rename_log_words(&mut history);
            let decoded: Log = serde_json::from_value(history.clone()).map_err(schema)?;
            let expected_session = (session != &self.project_id).then_some(session.as_str());
            if decoded
                .log_id
                .as_deref()
                .is_some_and(|id| Some(id) != expected_session)
                || decoded
                    .metadata
                    .as_ref()
                    .and_then(|m| m.project_id.as_deref())
                    .is_some_and(|id| id != self.project_id)
            {
                return Err(identity(
                    "The saved reasoning belongs to another project or session.",
                ));
            }
            history["log_id"] = json!(expected_session);
            if history["metadata"].is_null() {
                history["metadata"] = json!({});
            }
            history["metadata"]["project_id"] = json!(self.project_id);
            for (index, note) in decoded.notes.iter().enumerate() {
                if note.record_id.is_none() {
                    let identity = format!("{}\0{session}\0{}", self.project_id, note.note);
                    history["notes"][index]["record_id"] = json!(format!(
                        "upgraded:{}",
                        hex::encode(Sha256::digest(identity.as_bytes()))
                    ));
                }
            }
            let logs = project["logs"]
                .as_object_mut()
                .expect("validated project logs");
            if let Some(existing) = logs.get(session) {
                if existing != &history {
                    return Err(identity(
                        "Two different copies of a reasoning session were found; both originals are unchanged.",
                    ));
                }
            } else {
                logs.insert(session.clone(), history);
            }
        }
        crate::infra::CrossRef::upgrade_stored_refs(
            &mut project["logs"],
            StoredRefWords::BeforeRunWords,
        );
        let saved: PersistedProject = serde_json::from_value(project.clone()).map_err(schema)?;
        self.validate(&saved)?;
        crate::infra::versioned_document(project, SCHEMA_VERSION).map_err(schema)
    }
}

/// Move a saved project from the reasoning words to the decision words: its
/// histories become logs, each with notes and threads.
fn rename_project_words(project: &mut Value) {
    rename_key(project, "histories", "logs");
    if let Some(logs) = project["logs"].as_object_mut() {
        logs.values_mut().for_each(rename_log_words);
    }
}

/// One saved log: its steps become notes, its branches threads, and an open
/// branch an open thread.
fn rename_log_words(log: &mut Value) {
    rename_key(log, "steps", "notes");
    rename_key(log, "branches", "threads");
    rename_key(log, "session_id", "log_id");
    rename_key(&mut log["metadata"], "branches_created", "threads_created");
    for note in each_mut(&mut log["notes"]) {
        rename_note_words(note);
    }
    for thread in each_mut(&mut log["threads"]) {
        rename_key(thread, "from_step", "from_note");
        rename_key(thread, "steps", "notes");
        if thread["status"] == "active" {
            thread["status"] = json!("open");
        }
        for note in each_mut(&mut thread["notes"]) {
            rename_note_words(note);
        }
    }
}

fn rename_note_words(note: &mut Value) {
    for (old, new) in [
        ("step_number", "note"),
        ("revises_step", "revises_note"),
        ("is_final_step", "is_final_note"),
        ("branch_from", "thread_from"),
        ("branch_id", "thread_id"),
        ("branch_name", "thread_name"),
        ("session_id", "log_id"),
        ("execution_ref", "run_ref"),
    ] {
        rename_key(note, old, new);
    }
    for edge in each_mut(&mut note["dependencies"]) {
        rename_key(edge, "step", "note");
    }
}

fn each_mut(value: &mut Value) -> impl Iterator<Item = &mut Value> {
    value.as_array_mut().into_iter().flatten()
}

fn rename_key(value: &mut Value, old: &str, new: &str) {
    if let Some(fields) = value.as_object_mut()
        && let Some(saved) = fields.remove(old)
    {
        fields.insert(new.to_owned(), saved);
    }
}

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "persistence_upgrade.tests.rs"]
mod tests;
