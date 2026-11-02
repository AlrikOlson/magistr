//! Convert recognized saved-work envelopes before the admitted owner publishes them.
use super::{ExecutionIdentity, Goal, Persistence, ReadError, ReadErrorKind, SCHEMA_VERSION, Step};
use serde_json::{Value, json};

impl Persistence {
    /// Convert a saved run document once: version 1 gains its execution
    /// identities, versions 1 and 2 take the run vocabulary (goal, steps,
    /// progress, waiting, done, finished), and versions 1 to 3 name plan
    /// tasks `task:` instead of `chunk:`. Current readers stay strict and
    /// never invent identities. The caller holds the owner/document locks and
    /// retains the original bytes.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        let version = value["schema_version"].as_u64();
        match version {
            Some(1..=3) => {
                // Run documents never stored run references as strings, so only
                // the plan task prefix changes here, whatever the version.
                crate::infra::CrossRef::upgrade_stored_refs(
                    &mut value,
                    crate::infra::StoredRefWords::BeforePlanWords,
                );
                if version != Some(3) {
                    rename_document_words(&mut value)?;
                }
                if version == Some(1) {
                    upgrade_cycle(&mut value, project_id)?;
                    let object = value
                        .as_object_mut()
                        .ok_or_else(|| schema("Expected saved work."))?;
                    object.entry("selection_revision").or_insert(json!(0));
                    object.entry("saved_cycles").or_insert(json!([]));
                    if let Some(cycles) =
                        object.get_mut("saved_cycles").and_then(Value::as_array_mut)
                    {
                        for cycle in cycles {
                            upgrade_cycle(cycle, project_id)?;
                        }
                    }
                }
                let bytes =
                    crate::infra::versioned_document(value, SCHEMA_VERSION).map_err(schema)?;
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

    /// Convert one stored check result to the run vocabulary: its target
    /// names a step (`step_id`, `step_instance`). Converting a current
    /// record changes nothing.
    pub fn upgrade_check_result(bytes: &[u8]) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        let record = value
            .as_object_mut()
            .ok_or_else(|| schema("Expected a saved check result."))?;
        let mut changed = false;
        if let Some(target) = record.get_mut("target").and_then(Value::as_object_mut) {
            changed |= rename_key(target, "task_id", "step_id")?;
            changed |= rename_key(target, "task_instance", "step_instance")?;
        }
        if !changed {
            return Ok(bytes.to_vec());
        }
        serde_json::to_vec(&value).map_err(schema)
    }
}

/// Rename the run family's stored words in a version 1 or 2 document: the
/// selected cycle and every saved cycle.
fn rename_document_words(value: &mut Value) -> Result<(), ReadError> {
    let document = value
        .as_object_mut()
        .ok_or_else(|| schema("Expected saved work."))?;
    rename_cycle_words(document)?;
    if let Some(cycles) = document
        .get_mut("saved_cycles")
        .and_then(Value::as_array_mut)
    {
        for cycle in cycles {
            let cycle = cycle
                .as_object_mut()
                .ok_or_else(|| schema("Expected a saved cycle."))?;
            rename_cycle_words(cycle)?;
        }
    }
    Ok(())
}

fn rename_cycle_words(cycle: &mut serde_json::Map<String, Value>) -> Result<(), ReadError> {
    rename_key(cycle, "objective", "goal")?;
    rename_key(cycle, "tasks", "steps")?;
    rename_key(cycle, "next_action_id", "next_progress_id")?;
    if let Some(goal) = cycle.get_mut("goal").and_then(Value::as_object_mut) {
        rename_status(goal, &[("defined", "open"), ("completed", "finished")]);
    }
    if let Some(execution) = cycle.get_mut("execution").and_then(Value::as_object_mut) {
        rename_key(execution, "tasks", "steps")?;
    }
    if let Some(steps) = cycle.get_mut("steps").and_then(Value::as_array_mut) {
        for step in steps {
            let step = step
                .as_object_mut()
                .ok_or_else(|| schema("Expected a saved step."))?;
            rename_status(step, &[("blocked", "waiting"), ("completed", "done")]);
            rename_key(step, "blocked_reason", "waiting_reason")?;
            rename_key(step, "actions", "progress")?;
            if let Some(progress) = step.get_mut("progress").and_then(Value::as_array_mut) {
                for entry in progress.iter_mut().filter_map(Value::as_object_mut) {
                    rename_key(entry, "task_id", "step_id")?;
                }
            }
            if let Some(checks) = step.get_mut("checks").and_then(Value::as_array_mut) {
                for check in checks.iter_mut().filter_map(Value::as_object_mut) {
                    rename_key(check, "receipt_id", "check_result_id")?;
                }
            }
        }
    }
    Ok(())
}

/// Move `old` to `new`, refusing a record that already holds both.
fn rename_key(
    object: &mut serde_json::Map<String, Value>,
    old: &str,
    new: &str,
) -> Result<bool, ReadError> {
    let Some(value) = object.remove(old) else {
        return Ok(false);
    };
    if object.contains_key(new) {
        return Err(schema(format!(
            "A saved record holds both {old} and {new}; the original is unchanged."
        )));
    }
    object.insert(new.to_owned(), value);
    Ok(true)
}

fn rename_status(object: &mut serde_json::Map<String, Value>, renames: &[(&str, &str)]) {
    if let Some(status) = object.get_mut("status")
        && let Some((_, new)) = renames.iter().find(|(old, _)| status.as_str() == Some(old))
    {
        *status = json!(new);
    }
}

fn upgrade_cycle(value: &mut Value, project_id: &str) -> Result<(), ReadError> {
    let goal: Option<Goal> = serde_json::from_value(value["goal"].clone()).map_err(schema)?;
    let steps: Vec<Step> = serde_json::from_value(value["steps"].clone()).map_err(schema)?;
    if let Some(steps) = value["steps"].as_array_mut() {
        for step in steps {
            if let Some(progress) = step["progress"].as_array_mut() {
                for progress in progress {
                    if let Some(progress) = progress.as_object_mut()
                        && let Some(step) = progress.remove("deliberate_step")
                    {
                        if progress
                            .get("think_step")
                            .is_some_and(|current| current != &step)
                        {
                            return Err(schema(
                                "A saved progress refers to two different reasoning steps; the original is unchanged.",
                            ));
                        }
                        progress.insert("think_step".into(), step);
                    }
                }
            }
        }
    }
    if !value["execution"].is_null() {
        return Ok(());
    }
    let stamp = goal
        .as_ref()
        .and_then(|o| o.created_at.as_deref())
        .unwrap_or("unscoped");
    // Match the identities assigned by the older writer so retained check_results
    // still address the same run and step instances.
    let identity = ExecutionIdentity {
        run_id: format!("legacy:{project_id}:{stamp}"),
        steps: steps
            .iter()
            .enumerate()
            .map(|(index, step)| (step.id.clone(), format!("legacy:{index}:{}", step.id)))
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
