//! Serialize a converted saved document with its version first.
//!
//! Admission reads a document's `schema_version` from its first bytes, so a
//! document must name its version before anything else. Typed writers get
//! that from field order; a document converted as a JSON value does not,
//! because object keys serialize sorted. This writes the version first and
//! the rest of the document unchanged.

use serde_json::Value;

/// `value` serialized with `"schema_version": version` as its first key.
///
/// # Errors
/// When `value` is not a JSON object or cannot be serialized.
pub fn versioned_document(mut value: Value, version: u32) -> Result<Vec<u8>, String> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| "A saved document must be a JSON object.".to_owned())?;
    object.remove("schema_version");
    let rest = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    let mut bytes = format!("{{\"schema_version\":{version}").into_bytes();
    if rest.len() > 2 {
        bytes.push(b',');
    }
    bytes.extend_from_slice(&rest[1..]);
    Ok(bytes)
}

#[cfg(test)]
#[path = "versioned_document.tests.rs"]
mod tests;
