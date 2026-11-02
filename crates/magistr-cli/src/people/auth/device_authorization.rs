//! The provider's answer to the device-authorization request.

use serde_json::Value;

use crate::error::MagistrError;

/// RFC 8628 §3.2 — what the device gets back before the human has done
/// anything: a code to poll with, a code to read out, and where to type it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAuthorization {
    /// The secret the CLI polls the token endpoint with.
    pub device_code: String,
    /// The short code the human types into the browser.
    pub user_code: String,
    /// Where the human types it.
    pub verification_uri: String,
    /// The same page with the code pre-filled, when the provider offers it.
    pub verification_uri_complete: Option<String>,
    /// Seconds until `device_code` expires.
    pub expires_in: u64,
    /// Seconds the provider asks the CLI to wait between polls.
    pub interval: Option<u64>,
}

impl DeviceAuthorization {
    /// Read a device-authorization response.
    ///
    /// Accepts `verification_url` alongside the spec's
    /// `verification_uri` — Google shipped the draft spelling and never
    /// retired it. `expires_in` defaults to the spec's 15 minutes when a
    /// provider omits it.
    pub fn from_json(value: &Value) -> Result<Self, MagistrError> {
        let string = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        let device_code = string("device_code").ok_or_else(|| missing("device_code"))?;
        let user_code = string("user_code").ok_or_else(|| missing("user_code"))?;
        let verification_uri = string("verification_uri")
            .or_else(|| string("verification_url"))
            .ok_or_else(|| missing("verification_uri"))?;
        Ok(DeviceAuthorization {
            device_code,
            user_code,
            verification_uri,
            verification_uri_complete: string("verification_uri_complete")
                .or_else(|| string("verification_url_complete")),
            expires_in: value
                .get("expires_in")
                .and_then(Value::as_u64)
                .unwrap_or(900),
            interval: value.get("interval").and_then(Value::as_u64),
        })
    }
}

fn missing(field: &str) -> MagistrError {
    MagistrError::Other(format!(
        "device authorization response is missing '{field}'"
    ))
}

#[cfg(test)]
#[path = "device_authorization.tests.rs"]
mod tests;
