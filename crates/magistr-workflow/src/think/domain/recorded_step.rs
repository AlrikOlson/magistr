//! Native result of a reasoning mutation; display projection is not its identity.
use crate::think::domain::ThinkStep;

#[derive(Debug, Clone)]
pub struct RecordedStep {
    pub project_id: String,
    pub step: ThinkStep,
    /// The existing extensible trace display projection, produced without a
    /// serialization roundtrip. Native callers use `step` for identity.
    pub report: serde_json::Value,
}
