use serde::{Deserialize, Serialize};

/// One check_result a completion accepted as current evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedCheckResult {
    pub gate: String,
    pub check_result_id: String,
    pub attempt: u64,
    pub source_revision: Option<String>,
}
