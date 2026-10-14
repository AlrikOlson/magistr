use serde::{Deserialize, Serialize};

use super::{CheckRequest, SourceObservation, TaskAdmission};

/// An admitted check retains its exact task and request across asynchronous
/// work, together with the durable receipt it was admitted under, the
/// acceptance revision then current and the source observed at that moment.
/// Plain data: a late result carried through another process settles
/// against the same receipt id.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckAdmission {
    pub target: TaskAdmission,
    pub request: CheckRequest,
    pub receipt_id: String,
    pub acceptance_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceObservation>,
}
