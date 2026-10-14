//! Native result of a committed reasoning mutation.
use crate::ReadSource;
use crate::think::ThinkStep;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecordedStep {
    pub project_id: String,
    pub step: ThinkStep,
    pub source: ReadSource,
    /// Bounded display projection of the committed state.
    pub report: serde_json::Value,
}
