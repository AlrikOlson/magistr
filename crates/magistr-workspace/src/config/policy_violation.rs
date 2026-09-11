use super::ConfigLayer;
use serde::{Deserialize, Serialize};

/// A refused attempt to loosen an explicitly pinned workspace setting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyViolation {
    pub code: String,
    pub key: String,
    pub attempted_by: ConfigLayer,
    pub attempted: serde_json::Value,
    pub pinned_by: ConfigLayer,
    pub message: String,
}
