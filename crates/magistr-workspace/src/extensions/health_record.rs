use serde::{Deserialize, Serialize};

/// One circuit-breaker record: a breach keyed by plugin label and the sha256
/// of the component bytes it happened under. Changed bytes clear the breaker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginHealthRecord {
    pub plugin: String,
    pub component_hash: String,
    pub reason: String,
    /// RFC 3339 wall-clock of the breach.
    pub at: String,
}
