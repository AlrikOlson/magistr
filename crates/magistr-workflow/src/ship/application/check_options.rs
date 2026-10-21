use serde::{Deserialize, Serialize};

/// Inputs and execution controls for a bounded verification command.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct CheckOptions {
    /// Relative files/directories, including dependencies. Empty observes all source.
    pub inputs: Vec<String>,
    /// Reuse an eligible verified result; false always executes the command.
    pub reuse: bool,
    /// Per-command deadline. The host configuration supplies the default.
    pub timeout_secs: Option<u64>,
    /// Resolved by the host when the check is admitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parallel: Option<usize>,
    /// Shared resources; omitted conservatively infers the command's tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<String>>,
}
impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            reuse: true,
            timeout_secs: None,
            max_parallel: None,
            resources: None,
        }
    }
}

impl CheckOptions {
    pub const DEFAULT_TIMEOUT_SECS: u64 = 900;
    pub const MAX_TIMEOUT_SECS: u64 = 3600;
}
