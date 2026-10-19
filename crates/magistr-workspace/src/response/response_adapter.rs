use super::{Redactor, SealedResponse};
use crate::contracts::{EnvelopeVersion, ErrorKind, WorkspaceError, WorkspaceResponse};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// Stateless JSON adaptation. The native contract contains no JSON representation.
pub struct ResponseAdapter;

impl ResponseAdapter {
    pub const META_MAX_ENVELOPE_VERSION: &str = "magistr/maxEnvelopeVersion";
    pub const META_SESSION_ID: &str = "magistr/sessionId";
    /// Ceiling on a session identity, in UTF-16 units as retained, so a
    /// hostile client cannot bloat the journal's identity column.
    pub const SESSION_ID_MAX_UNITS: usize = 256;

    /// The caller's session identity from `_meta`: identity travels on the
    /// request and "" is the anonymous identity. Anything but a non-empty
    /// string within the ceiling is anonymous.
    #[must_use]
    pub fn session_meta(meta: Option<&Map<String, Value>>) -> String {
        meta.and_then(|meta| meta.get(Self::META_SESSION_ID))
            .and_then(Value::as_str)
            .filter(|session| {
                !session.is_empty() && session.encode_utf16().count() <= Self::SESSION_ID_MAX_UNITS
            })
            .map(str::to_owned)
            .unwrap_or_default()
    }

    pub fn negotiate_meta(
        meta: Option<&Map<String, Value>>,
    ) -> Result<EnvelopeVersion, WorkspaceError> {
        let Some(declared) = meta.and_then(|meta| meta.get(Self::META_MAX_ENVELOPE_VERSION)) else {
            return EnvelopeVersion::negotiate(None);
        };
        // JSON has one number category at the reference boundary, including 1.0
        // and integers greater than u64::MAX. Clamp only after validating the pin.
        if let Some(number) = declared.as_f64()
            && number.is_finite()
            && number >= 1.0
            && number.fract() == 0.0
        {
            return EnvelopeVersion::negotiate(Some(
                number.min(EnvelopeVersion::LATEST.get() as f64) as u64,
            ));
        }
        Err(WorkspaceError::new(
            ErrorKind::UnsupportedVersion,
            format!(
                "{} must be a positive integer, got {}",
                Self::META_MAX_ENVELOPE_VERSION,
                describe_declaration(declared)
            ),
        ))
    }

    pub fn seal<R: Serialize>(
        response: &WorkspaceResponse<R>,
    ) -> Result<SealedResponse, serde_json::Error> {
        let envelope = build(response, true)?;
        let json = serde_json::to_string(&envelope)?;
        Ok(SealedResponse { envelope, json })
    }

    /// The same envelope shape without redaction: what a script guest sees
    /// for its own workspace reads. It never crosses a transport; the guest's
    /// return value is redacted when the script envelope itself is sealed.
    pub fn envelope<R: Serialize>(
        response: &WorkspaceResponse<R>,
    ) -> Result<Value, serde_json::Error> {
        build(response, false)
    }
}

fn build<R: Serialize>(
    response: &WorkspaceResponse<R>,
    redact: bool,
) -> Result<Value, serde_json::Error> {
    let guard = |value: Value| if redact { redact_value(value) } else { value };
    Ok(match response {
        WorkspaceResponse::Success {
            version,
            freshness,
            budget,
            results,
            notes,
        } => {
            let mut envelope = json!({"v": version.get(), "ok": true,
                "freshness": freshness.as_str(),
                "budget": {"requested": budget.requested, "used": budget.used},
                "results": guard(serde_json::to_value(results.rows())?)});
            if let Some(cut) = results.truncation() {
                // The pinned response seam preserves cursor and typed hints verbatim.
                envelope["truncation"] = json!({"dropped": cut.dropped(),
                    "reason": cut.reason().as_str(), "narrow": cut.narrow(), "cursor": cut.cursor()});
            }
            add_notes(&mut envelope, notes, redact);
            envelope
        }
        WorkspaceResponse::Failure {
            version,
            error,
            notes,
        } => {
            let mut envelope = json!({"v": version.get(), "ok": false,
                "error": guard(error_value(error))});
            add_notes(&mut envelope, notes, redact);
            envelope
        }
    })
}

fn add_notes(envelope: &mut Value, notes: &Option<Vec<String>>, redact: bool) {
    if let Some(notes) = notes {
        envelope["notes"] = Value::Array(
            notes
                .iter()
                .map(|note| {
                    Value::String(if redact {
                        Redactor::redact(note).text
                    } else {
                        note.clone()
                    })
                })
                .collect(),
        );
    }
}

fn redact_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(Redactor::redact(&text).text),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_value).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, redact_value(value)))
                .collect(),
        ),
        value => value,
    }
}

fn error_value(error: &WorkspaceError) -> Value {
    let mut value = json!({"code": error.code(), "recovery": error.recovery().as_str(), "message": error.message});
    match &error.kind {
        ErrorKind::AmbiguousSymbol { candidates } => value["candidates"] = json!(candidates),
        ErrorKind::ParseRejected {
            parser_message,
            rejected_diff,
        } => {
            value["parserMessage"] = json!(parser_message);
            value["rejectedDiff"] = json!(rejected_diff);
        }
        ErrorKind::StaleAnchor { content_hash } => value["contentHash"] = json!(content_hash),
        ErrorKind::ConfirmRequired { confirm_token } => {
            value["confirmToken"] = json!(confirm_token)
        }
        ErrorKind::EngineBusy { backoff_ms } => value["backoffMs"] = json!(backoff_ms),
        ErrorKind::PluginCrashed { plugin } => value["plugin"] = json!(plugin),
        _ => {}
    }
    value
}

// Refusal messages are part of the pinned observable contract. JSON.stringify
// uses decimal notation for [1e-6, 1e21), emits a plus sign on positive exponents,
// and normalizes negative zero. This stays at the untyped metadata boundary.
fn describe_declaration(value: &Value) -> String {
    match value {
        Value::Number(number) => {
            let Some(number) = number.as_f64() else {
                return number.to_string();
            };
            if number == 0.0 {
                return "0".into();
            }
            if (1e-6..1e21).contains(&number.abs()) {
                return number.to_string();
            }
            let scientific = format!("{number:e}");
            if let Some((mantissa, exponent)) = scientific.split_once('e') {
                let sign = if exponent.starts_with('-') { "" } else { "+" };
                format!("{mantissa}e{sign}{exponent}")
            } else {
                scientific
            }
        }
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(describe_declaration)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => describe_object(values),
        value => value.to_string(),
    }
}

// Ordinary JS objects enumerate canonical array-index names numerically first,
// then other string names in insertion order. u32::MAX is not an array index.
fn describe_object(values: &Map<String, Value>) -> String {
    let mut entries: Vec<_> = values.iter().collect();
    entries.sort_by_key(|(key, _)| {
        let index = key
            .parse::<u32>()
            .ok()
            .filter(|index| *index != u32::MAX && index.to_string() == **key);
        (index.is_none(), index.unwrap_or_default())
    });
    format!(
        "{{{}}}",
        entries
            .into_iter()
            .map(|(key, value)| format!(
                "{}:{}",
                Value::String(key.clone()),
                describe_declaration(value)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}
