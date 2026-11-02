use serde::{Deserialize, Serialize};

use super::{AcceptedCheckResult, OutstandingGate, RecoveredFailure};

/// What a completion was decided on: the run and the verification step it
/// named, the source revision, the external configuration digest and the
/// acceptance revision that were current, every check_result accepted, every
/// historical failure a later pass recovered, and every gate still
/// outstanding. Complete when nothing is outstanding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionEvidence {
    pub run_id: String,
    pub step_id: String,
    pub step_instance: String,
    pub source_revision: String,
    #[serde(default)]
    pub config_digest: String,
    pub acceptance_revision: String,
    pub accepted: Vec<AcceptedCheckResult>,
    pub recovered: Vec<RecoveredFailure>,
    pub outstanding: Vec<OutstandingGate>,
}

impl CompletionEvidence {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.outstanding.is_empty()
    }
}
