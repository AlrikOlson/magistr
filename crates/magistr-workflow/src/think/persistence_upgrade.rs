//! Consolidate older per-session histories into the current project document.
use super::{
    PersistedProject, Persistence, ReadError, ReadErrorKind, SCHEMA_VERSION, ThinkHistory, identity,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

impl Persistence {
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
            self.validate_session_id(session)?;
            let value: Value = serde_json::from_slice(bytes).map_err(schema)?;
            match value["schema_version"].as_u64() {
                Some(1) => legacy.push((session, value)),
                Some(version)
                    if version == u64::from(SCHEMA_VERSION) && session == &self.project_id =>
                {
                    let saved: PersistedProject =
                        serde_json::from_value(value.clone()).map_err(schema)?;
                    self.validate(&saved)?;
                    project = value;
                    current = Some(bytes);
                }
                _ => {
                    return Err(schema(
                        "The saved reasoning uses an unsupported format; the original is unchanged.",
                    ));
                }
            }
        }
        if legacy.is_empty() {
            return current
                .cloned()
                .ok_or_else(|| schema("No saved reasoning was selected."));
        }
        for (session, value) in legacy {
            let mut history = value["history"].clone();
            let decoded: ThinkHistory = serde_json::from_value(history.clone()).map_err(schema)?;
            let expected_session = (session != &self.project_id).then_some(session.as_str());
            if decoded
                .session_id
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
            history["session_id"] = json!(expected_session);
            if history["metadata"].is_null() {
                history["metadata"] = json!({});
            }
            history["metadata"]["project_id"] = json!(self.project_id);
            for (index, step) in decoded.steps.iter().enumerate() {
                if step.record_id.is_none() {
                    let identity = format!("{}\0{session}\0{}", self.project_id, step.step_number);
                    history["steps"][index]["record_id"] = json!(format!(
                        "upgraded:{}",
                        hex::encode(Sha256::digest(identity.as_bytes()))
                    ));
                }
            }
            let histories = project["histories"]
                .as_object_mut()
                .expect("validated project histories");
            if let Some(existing) = histories.get(session) {
                if existing != &history {
                    return Err(identity(
                        "Two different copies of a reasoning session were found; both originals are unchanged.",
                    ));
                }
            } else {
                histories.insert(session.clone(), history);
            }
        }
        let saved: PersistedProject = serde_json::from_value(project.clone()).map_err(schema)?;
        self.validate(&saved)?;
        serde_json::to_vec(&project).map_err(schema)
    }
}

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "persistence_upgrade.tests.rs"]
mod tests;
