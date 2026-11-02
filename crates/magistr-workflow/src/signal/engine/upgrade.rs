//! Upgrade older project records through their owning family's validator.
use super::{SignalEngine, Signals, validate};
use crate::infra::{CrossRef, StoredRefWords, versioned_document};
use magistr_workflow_contracts::{ReadError, ReadErrorKind, SIGNALS_SCHEMA_VERSION};
use serde_json::{Value, json};

impl SignalEngine {
    /// Convert a saved signal document once. A document without a version
    /// was written before the plan words: its references take them (and the
    /// run words they may still spell) and their field becomes `refs`.
    /// Version 2 was written before the decision words: enrichment points at
    /// its decision `note` and notes are referenced as `decision:`.
    /// Omitted item origins are filled from the enclosing project's identity
    /// during admitted relocation; explicitly different identities remain
    /// conflicts. A current document is returned unchanged.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        let mut changed = match value.get("schema_version").and_then(Value::as_u64) {
            None => {
                let document = value
                    .as_object_mut()
                    .ok_or_else(|| schema("Expected saved signals."))?;
                for signal in document
                    .get_mut("signals")
                    .and_then(Value::as_array_mut)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_object_mut)
                {
                    rename_key(signal, "cross_refs", "refs")?;
                }
                rename_decision_words(&mut value)?;
                CrossRef::upgrade_stored_refs(&mut value, StoredRefWords::BeforeRunWords);
                value["schema_version"] = json!(SIGNALS_SCHEMA_VERSION);
                true
            }
            Some(2) => {
                rename_decision_words(&mut value)?;
                CrossRef::upgrade_stored_refs(&mut value, StoredRefWords::BeforeDecisionWords);
                value["schema_version"] = json!(SIGNALS_SCHEMA_VERSION);
                true
            }
            Some(version) if version == u64::from(SIGNALS_SCHEMA_VERSION) => false,
            Some(_) => {
                return Err(schema(
                    "The saved signals use an unsupported format; the original is unchanged.",
                ));
            }
        };
        let mut saved: Signals = serde_json::from_value(value.clone()).map_err(schema)?;
        if saved.project_id != project_id {
            validate(&saved, project_id)?;
        }
        for (index, item) in saved.signals.iter_mut().enumerate() {
            if item.project_id.is_none() {
                item.project_id = Some(project_id.to_owned());
                value["signals"][index]["project_id"] = json!(project_id);
                changed = true;
            }
        }
        validate(&saved, project_id)?;
        if changed {
            versioned_document(value, SIGNALS_SCHEMA_VERSION).map_err(schema)
        } else {
            Ok(bytes.to_vec())
        }
    }
}

/// Each enrichment points at the decision note that motivated it as `note`.
fn rename_decision_words(value: &mut Value) -> Result<(), ReadError> {
    let signals = value
        .get_mut("signals")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut);
    for signal in signals {
        let enrichment = signal
            .get_mut("enrichment")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
            .filter_map(Value::as_object_mut);
        for entry in enrichment {
            rename_key(entry, "think_step", "note")?;
        }
    }
    Ok(())
}

/// Move `old` to `new`, refusing a record that already holds both.
fn rename_key(
    object: &mut serde_json::Map<String, Value>,
    old: &str,
    new: &str,
) -> Result<(), ReadError> {
    let Some(value) = object.remove(old) else {
        return Ok(());
    };
    if object.contains_key(new) {
        return Err(schema(format!(
            "A saved signal holds both {old} and {new}; the original is unchanged."
        )));
    }
    object.insert(new.to_owned(), value);
    Ok(())
}

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "upgrade.tests.rs"]
mod tests;
