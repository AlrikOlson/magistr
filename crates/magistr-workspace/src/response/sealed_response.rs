use serde_json::Value;

/// Immutable structured response and its matching serialized bytes.
#[derive(Debug, Clone)]
pub struct SealedResponse {
    pub(super) envelope: Value,
    pub(super) json: String,
}

impl SealedResponse {
    #[must_use]
    pub fn structured(&self) -> &Value {
        &self.envelope
    }
    #[must_use]
    pub fn json(&self) -> &str {
        &self.json
    }
}
