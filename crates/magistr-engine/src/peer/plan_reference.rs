use serde::{Deserialize, Serialize};

/// Exact consent target. A hash without its project/run/revision is not an approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanReference {
    pub project_id: String,
    pub run_id: String,
    pub revision: u64,
    pub digest: String,
}
