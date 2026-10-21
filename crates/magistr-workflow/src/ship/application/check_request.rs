use serde::{Deserialize, Serialize};

use super::CheckOptions;
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
