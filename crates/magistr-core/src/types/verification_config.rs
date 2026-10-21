use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Execution and input contracts for configured checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct VerificationConfig {
    /// Per-command deadline, shared by tools and the iteration runner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    /// Maximum independent checks in flight. Resource locks still apply.
    pub max_parallel: usize,
    /// Overrides keyed by configured gate name.
    pub gates: BTreeMap<String, GatePolicy>,
}
impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            timeout_secs: None,
            max_parallel: 4,
            gates: BTreeMap::new(),
        }
    }
}
impl VerificationConfig {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

/// A declared dependency boundary. Omitted inputs mean the whole source tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GatePolicy {
    pub inputs: Vec<String>,
    /// Whether a successful execution may be reused with identical inputs.
    pub reuse: Option<bool>,
    pub timeout_secs: Option<u64>,
    /// Empty means independent. Omitted means infer known tools or serialize
    /// an opaque command against all work in this workspace.
    pub resources: Option<Vec<String>>,
}
