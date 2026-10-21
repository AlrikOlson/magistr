//! Upgrade older project records through their owning family's validator.
use super::{Roadmap, RoadmapEngine, observation};
use magistr_workflow_contracts::{ReadError, ReadErrorKind};
use serde_json::{Value, json};

impl RoadmapEngine {
    /// Fill omitted item origins from the enclosing project's identity during
    /// admitted relocation. Explicitly different identities remain conflicts.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        let mut saved: Roadmap = serde_json::from_value(value.clone()).map_err(schema)?;
        if saved.project_id != project_id {
            observation::validate(&saved, project_id)?;
        }
        let mut changed = false;
        for (index, item) in saved.chunks.iter_mut().enumerate() {
            if item.project_id.is_none() {
                item.project_id = Some(project_id.to_owned());
                value["chunks"][index]["project_id"] = json!(project_id);
                changed = true;
            }
        }
        observation::validate(&saved, project_id)?;
        if changed {
            serde_json::to_vec(&value).map_err(schema)
        } else {
            Ok(bytes.to_vec())
        }
    }
}

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "upgrade.tests.rs"]
mod tests;
