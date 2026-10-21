//! Convert recognized saved-work envelopes before the admitted owner publishes them.
use super::{
    ExecutionIdentity, Objective, Persistence, ReadError, ReadErrorKind, SCHEMA_VERSION, Task,
};
use serde_json::{Value, json};

impl Persistence {
    /// Convert version 1 once; current readers remain strict and never invent identities.
    /// The caller holds the owner/document locks and retains the original bytes.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        match value["schema_version"].as_u64() {
            Some(1) => {
                upgrade_cycle(&mut value, project_id)?;
                let object = value
                    .as_object_mut()
                    .ok_or_else(|| schema("Expected saved work."))?;
                object.entry("selection_revision").or_insert(json!(0));
                object.entry("saved_cycles").or_insert(json!([]));
                if let Some(cycles) = object.get_mut("saved_cycles").and_then(Value::as_array_mut) {
                    for cycle in cycles {
                        upgrade_cycle(cycle, project_id)?;
                    }
                }
                value["schema_version"] = json!(SCHEMA_VERSION);
                let bytes = serde_json::to_vec(&value).map_err(schema)?;
                Self::validate_document(&bytes, project_id)?;
                Ok(bytes)
            }
            Some(version) if version == u64::from(SCHEMA_VERSION) => {
                Self::validate_document(bytes, project_id)?;
                Ok(bytes.to_vec())
            }
            _ => Err(schema(
                "The saved work uses an unsupported format; the original is unchanged.",
            )),
        }
    }
}

fn upgrade_cycle(value: &mut Value, project_id: &str) -> Result<(), ReadError> {
    let objective: Option<Objective> =
        serde_json::from_value(value["objective"].clone()).map_err(schema)?;
    let tasks: Vec<Task> = serde_json::from_value(value["tasks"].clone()).map_err(schema)?;
    if let Some(tasks) = value["tasks"].as_array_mut() {
        for task in tasks {
            if let Some(actions) = task["actions"].as_array_mut() {
                for action in actions {
                    if let Some(action) = action.as_object_mut()
                        && let Some(step) = action.remove("deliberate_step")
                    {
                        if action
                            .get("think_step")
                            .is_some_and(|current| current != &step)
                        {
                            return Err(schema(
                                "A saved action refers to two different reasoning steps; the original is unchanged.",
                            ));
                        }
                        action.insert("think_step".into(), step);
                    }
                }
            }
        }
    }
    if !value["execution"].is_null() {
        return Ok(());
    }
    let stamp = objective
        .as_ref()
        .and_then(|o| o.created_at.as_deref())
        .unwrap_or("unscoped");
    // Match the identities assigned by the older writer so retained receipts
    // still address the same run and task instances.
    let identity = ExecutionIdentity {
        run_id: format!("legacy:{project_id}:{stamp}"),
        tasks: tasks
            .iter()
            .enumerate()
            .map(|(index, task)| (task.id.clone(), format!("legacy:{index}:{}", task.id)))
            .collect(),
    };
    value["execution"] = serde_json::to_value(identity).map_err(schema)?;
    Ok(())
}

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "persistence_upgrade.tests.rs"]
mod tests;
