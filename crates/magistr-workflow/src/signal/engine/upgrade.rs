//! Upgrade older project records through their owning family's validator.
use super::{SignalEngine, Signals, validate};
use crate::infra::{CrossRef, StoredRefWords, versioned_document};
use magistr_workflow_contracts::{ReadError, ReadErrorKind, SIGNALS_SCHEMA_VERSION};
use serde_json::{Value, json};

impl SignalEngine {
    /// Convert a saved signal document once. A document without a version
    /// was written before the plan words: its references take them (and the
    /// run words they may still spell) and their field becomes `refs`.
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
                    if let Some(refs) = signal.remove("cross_refs") {
                        if signal.contains_key("refs") {
                            return Err(schema(
                                "A saved signal holds both cross_refs and refs; the original is unchanged.",
                            ));
                        }
                        signal.insert("refs".into(), refs);
                    }
                }
                CrossRef::upgrade_stored_refs(&mut value, StoredRefWords::BeforeRunWords);
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

fn schema(error: impl std::fmt::Display) -> ReadError {
    ReadError::new(ReadErrorKind::Schema, error.to_string())
}

#[cfg(test)]
#[path = "upgrade.tests.rs"]
mod tests;
