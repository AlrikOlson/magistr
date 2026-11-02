use serde::{Deserialize, Serialize};

use super::{CheckRequest, SourceObservation, StepAdmission};

/// An admitted check retains its exact step and request across asynchronous
/// work, together with the durable check_result it was admitted under, the
/// acceptance revision then current and the source observed at that moment.
/// Plain data: a late result carried through another process settles
/// against the same check_result id.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckAdmission {
    pub target: StepAdmission,
    pub request: CheckRequest,
    pub check_result_id: String,
    pub acceptance_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceObservation>,
}
