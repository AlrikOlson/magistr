use serde::{Deserialize, Serialize};

/// Per-guard enable/disable flags in project configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuardsConfig {
    /// Enable the plan guard (default: true).
    #[serde(default = "default_true")]
    pub plan: bool,
    /// Enable the spec guard (default: true).
    #[serde(default = "default_true")]
    pub spec: bool,
    /// Enable the clean tree guard (default: true).
    #[serde(default = "default_true")]
    pub clean_tree: bool,
    /// Enable the quality gate guard (default: true).
    #[serde(default = "default_true")]
    pub quality_gate: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GuardsConfig {
    fn default() -> Self {
        Self {
            plan: true,
            spec: true,
            clean_tree: true,
            quality_gate: true,
        }
    }
}
