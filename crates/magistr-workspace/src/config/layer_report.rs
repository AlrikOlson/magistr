use super::ConfigLayer;
use serde::{Deserialize, Serialize};

/// Whether a layer contributed settings, including a skipped layer's diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerReport {
    pub layer: ConfigLayer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
