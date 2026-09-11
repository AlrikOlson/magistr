use super::ConfigLayer;
use serde::{Deserialize, Serialize};

/// Effective value and the layer that decided (and optionally pinned) it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedKey {
    pub key: String,
    pub value: serde_json::Value,
    pub decided_by: ConfigLayer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_by: Option<ConfigLayer>,
}
