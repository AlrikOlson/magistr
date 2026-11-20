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
    /// Overrides keyed by configured check name.
    pub checks: BTreeMap<String, CheckPolicy>,
    /// Environment variables the project's checks read, beyond the ones that
    /// choose and steer their tools. A result is reused only while these keep
    /// their values. A trailing `*` names every variable with that prefix.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
}
impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            timeout_secs: None,
            max_parallel: 4,
            checks: BTreeMap::new(),
            env: Vec::new(),
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
pub struct CheckPolicy {
    pub inputs: Vec<String>,
    /// Whether a successful execution may be reused with identical inputs.
    pub reuse: Option<bool>,
    pub timeout_secs: Option<u64>,
    /// Empty means independent. Omitted means infer known tools or serialize
    /// an opaque command against all work in this workspace.
    pub resources: Option<Vec<String>>,
}
