use serde::{Deserialize, Serialize};

use crate::ship::domain::check::CheckType;

/// A check specification before either task admission or command execution.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckRequest {
    #[serde(flatten)]
    pub options: CheckOptions,
    pub task_id: Option<String>,
    pub check_type: CheckType,
    pub name: String,
    pub passed: Option<bool>,
    pub details: String,
    pub required: bool,
    pub command: Option<String>,
    /// Report format and path; parsing remains additive to the process result.
    pub report: Option<(String, String)>,
}

pub const DEFAULT_CHECK_TIMEOUT_SECS: u64 = 900;
pub const MAX_CHECK_TIMEOUT_SECS: u64 = 3600;

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
}
impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            reuse: true,
            timeout_secs: None,
        }
    }
}
