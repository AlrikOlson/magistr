//! Upgrade older project records through their owning family's validator.
use super::{Plan, PlanEngine, observation};
use crate::infra::{CrossRef, StoredRefWords, versioned_document};
use magistr_workflow_contracts::{PLAN_SCHEMA_VERSION, ReadError, ReadErrorKind};
use serde_json::{Map, Value, json};

impl PlanEngine {
    /// Convert a saved plan document once. A document without a version was
    /// written before the plan words: it takes them here (tasks, features,
    /// waiting, retired, `task:` references) together with the run words its
    /// references may still spell. Version 2 was written before the decision
    /// words: refresh records point at their decision `notes` and notes are
    /// referenced as `decision:`. Version 3 was written before the request
    /// words: requests are referenced as `request:`. Version 4 was written
    /// before the team words: a focus names its `team`. Each older version
    /// takes every later step in order. Omitted item origins are filled from
    /// the enclosing project's identity; explicitly different identities
    /// remain conflicts. A current document is returned unchanged, so the
    /// conversion is idempotent; the version, never the references, decides
    /// what is old.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        let saved_version = value.get("schema_version").and_then(Value::as_u64);
        let mut changed = match saved_version {
            None | Some(2..=4) => {
                if saved_version.is_none() {
                    rename_plan_words(&mut value)?;
                }
                if saved_version.is_none_or(|version| version <= 2) {
                    rename_decision_words(&mut value)?;
                }
                let words = match saved_version {
                    None => Some(StoredRefWords::BeforeRunWords),
                    Some(2) => Some(StoredRefWords::BeforeDecisionWords),
                    Some(3) => Some(StoredRefWords::BeforeRequestWords),
                    _ => None,
                };
                if let Some(words) = words {
                    CrossRef::upgrade_stored_refs(&mut value, words);
                }
                rename_team_words(&mut value)?;
                value["schema_version"] = json!(PLAN_SCHEMA_VERSION);
                true
            }
            Some(version) if version == u64::from(PLAN_SCHEMA_VERSION) => false,
            Some(_) => {
                return Err(schema(
                    "The saved plan uses an unsupported format; the original is unchanged.",
                ));
            }
        };
        let mut saved: Plan = serde_json::from_value(value.clone()).map_err(schema)?;
        if saved.project_id != project_id {
            observation::validate(&saved, project_id)?;
        }
        for (index, item) in saved.tasks.iter_mut().enumerate() {
            if item.project_id.is_none() {
                item.project_id = Some(project_id.to_owned());
                value["tasks"][index]["project_id"] = json!(project_id);
                changed = true;
            }
        }
        observation::validate(&saved, project_id)?;
        if changed {
            // Admission reads the version from the first bytes, as the
            // writer serializes it.
            versioned_document(value, PLAN_SCHEMA_VERSION).map_err(schema)
        } else {
            Ok(bytes.to_vec())
        }
    }
}

const TASK_STATUSES: &[(&str, &str)] = &[
    ("backlog", "idea"),
    ("pending", "to_do"),
    ("blocked", "waiting"),
    ("obsoleted", "retired"),
];

const WAITING_REASONS: &[(&str, &str)] = &[
    ("awaiting_human", "person"),
    ("external", "outside"),
    ("premise_unmet", "condition"),
    ("premise_refuted", "disproved"),
];

/// Rename the plan family's stored words in a document saved before them.
fn rename_plan_words(value: &mut Value) -> Result<(), ReadError> {
    let document = value
        .as_object_mut()
        .ok_or_else(|| schema("Expected a saved plan."))?;
    rename_key(document, "chunks", "tasks")?;
    for task in objects(document.get_mut("tasks")) {
        rename_value(task, "status", TASK_STATUSES);
        rename_key(task, "group", "feature")?;
        rename_key(task, "cross_refs", "refs")?;
        rename_key(task, "ship_ref", "step_ref")?;
        rename_key(task, "obsoleted_reason", "retired_reason")?;
        rename_key(task, "blocked_by", "waiting_on")?;
        if let Some(waiting) = task.get_mut("waiting_on").and_then(Value::as_object_mut) {
            rename_value(waiting, "kind", WAITING_REASONS);
            rename_key(waiting, "blocked_at", "waiting_since")?;
        }
        if let Some(proposal) = task
            .get_mut("status_proposal")
            .and_then(Value::as_object_mut)
        {
            rename_value(proposal, "suggested_status", TASK_STATUSES);
        }
    }
    for key in ["links", "tracker_opt_ins"] {
        for link in objects(document.get_mut(key)) {
            rename_key(link, "chunk_id", "task_id")?;
        }
    }
    for container in objects(document.get_mut("container_links")) {
        rename_value(container, "kind", &[("group", "feature")]);
    }
    for focus in objects(document.get_mut("focuses")) {
        rename_key(focus, "group", "feature")?;
    }
    Ok(())
}

/// A refresh record lists the decision notes that motivated it as `notes`.
fn rename_decision_words(value: &mut Value) -> Result<(), ReadError> {
    let document = value
        .as_object_mut()
        .ok_or_else(|| schema("Expected a saved plan."))?;
    for refresh in objects(document.get_mut("refreshes")) {
        rename_key(refresh, "think_steps", "notes")?;
    }
    Ok(())
}

/// A focus names the team that holds it.
fn rename_team_words(value: &mut Value) -> Result<(), ReadError> {
    let document = value
        .as_object_mut()
        .ok_or_else(|| schema("Expected a saved plan."))?;
    for focus in objects(document.get_mut("focuses")) {
        rename_key(focus, "lane", "team")?;
    }
    Ok(())
}

fn objects(value: Option<&mut Value>) -> impl Iterator<Item = &mut Map<String, Value>> {
    value
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut)
}

/// Move `old` to `new`, refusing a record that already holds both.
fn rename_key(object: &mut Map<String, Value>, old: &str, new: &str) -> Result<(), ReadError> {
    let Some(value) = object.remove(old) else {
        return Ok(());
    };
    if object.contains_key(new) {
        return Err(schema(format!(
            "A saved record holds both {old} and {new}; the original is unchanged."
        )));
    }
    object.insert(new.to_owned(), value);
    Ok(())
}

fn rename_value(object: &mut Map<String, Value>, key: &str, renames: &[(&str, &str)]) {
    if let Some(slot) = object.get_mut(key)
        && let Some((_, new)) = renames.iter().find(|(old, _)| slot.as_str() == Some(old))
    {
        *slot = json!(new);
    }
}

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "upgrade.tests.rs"]
mod tests;
