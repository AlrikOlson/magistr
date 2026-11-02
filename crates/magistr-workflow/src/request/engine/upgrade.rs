//! Upgrade older project records through their owning family's validator.
//!
//! Saved request documents keep the words of the release that wrote them
//! until this codec converts them, so the old spellings below name what is
//! converted: the `signals` list, kind `idea`, status `surfaced`, the
//! `surfaced_at` stamp and `signal:` references.
use super::{RequestEngine, Requests, validate};
use crate::infra::{CrossRef, StoredRefWords, versioned_document};
use magistr_workflow_contracts::{REQUESTS_SCHEMA_VERSION, ReadError, ReadErrorKind};
use serde_json::{Map, Value, json};

impl RequestEngine {
    /// Convert a saved request document once. A document without a version
    /// was written before the plan words: its references take them (and the
    /// run words they may still spell) and their field becomes `refs`.
    /// Version 2 was written before the decision words: enrichment points at
    /// its decision `note` and notes are referenced as `decision:`. Version 3
    /// was written before the request words: its `signals` become
    /// `requests`, kind `idea` becomes `suggestion`, status `surfaced`
    /// becomes `raised` (and its stamp `raised_at`), and `signal:`
    /// references become `request:`.
    /// Omitted item origins are filled from the enclosing project's identity
    /// during admitted relocation; explicitly different identities remain
    /// conflicts. A current document is returned unchanged.
    pub fn upgrade_document(bytes: &[u8], project_id: &str) -> Result<Vec<u8>, ReadError> {
        let mut value: Value = serde_json::from_slice(bytes).map_err(schema)?;
        let words = match value.get("schema_version").and_then(Value::as_u64) {
            None => Some(StoredRefWords::BeforeRunWords),
            Some(2) => Some(StoredRefWords::BeforeDecisionWords),
            Some(3) => Some(StoredRefWords::BeforeRequestWords),
            Some(version) if version == u64::from(REQUESTS_SCHEMA_VERSION) => None,
            Some(_) => {
                return Err(schema(
                    "The saved requests use an unsupported format; the original is unchanged.",
                ));
            }
        };
        let mut changed = words.is_some();
        if let Some(words) = words {
            let document = value
                .as_object_mut()
                .ok_or_else(|| schema("Expected saved requests."))?;
            rename_key(document, "signals", "requests")?;
            for item in items(&mut value) {
                if words == StoredRefWords::BeforeRunWords {
                    rename_key(item, "cross_refs", "refs")?;
                }
                if words != StoredRefWords::BeforeRequestWords {
                    rename_decision_words(item)?;
                }
                rename_request_words(item)?;
            }
            CrossRef::upgrade_stored_refs(&mut value, words);
            value["schema_version"] = json!(REQUESTS_SCHEMA_VERSION);
        }
        let mut saved: Requests = serde_json::from_value(value.clone()).map_err(schema)?;
        if saved.project_id != project_id {
            validate(&saved, project_id)?;
        }
        for (index, item) in saved.requests.iter_mut().enumerate() {
            if item.project_id.is_none() {
                item.project_id = Some(project_id.to_owned());
                value["requests"][index]["project_id"] = json!(project_id);
                changed = true;
            }
        }
        validate(&saved, project_id)?;
        if changed {
            versioned_document(value, REQUESTS_SCHEMA_VERSION).map_err(schema)
        } else {
            Ok(bytes.to_vec())
        }
    }
}

/// Every saved request in the document.
fn items(value: &mut Value) -> impl Iterator<Item = &mut Map<String, Value>> {
    value
        .get_mut("requests")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut)
}

/// Each enrichment points at the decision note that motivated it as `note`.
fn rename_decision_words(request: &mut Map<String, Value>) -> Result<(), ReadError> {
    let enrichment = request
        .get_mut("enrichment")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut);
    for entry in enrichment {
        rename_key(entry, "think_step", "note")?;
    }
    Ok(())
}

/// The request words: kind `suggestion`, status `raised` and its stamp.
fn rename_request_words(request: &mut Map<String, Value>) -> Result<(), ReadError> {
    if request.get("kind").and_then(Value::as_str) == Some("idea") {
        request.insert("kind".into(), json!("suggestion"));
    }
    if request.get("status").and_then(Value::as_str) == Some("surfaced") {
        request.insert("status".into(), json!("raised"));
    }
    rename_key(request, "surfaced_at", "raised_at")
}

/// Move `old` to `new`, refusing a record that already holds both.
fn rename_key(object: &mut Map<String, Value>, old: &str, new: &str) -> Result<(), ReadError> {
    let Some(value) = object.remove(old) else {
        return Ok(());
    };
    if object.contains_key(new) {
        return Err(schema(format!(
            "A saved request holds both {old} and {new}; the original is unchanged."
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
