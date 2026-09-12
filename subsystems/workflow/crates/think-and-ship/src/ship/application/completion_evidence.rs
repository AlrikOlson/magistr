use serde::{Deserialize, Serialize};

use super::{AcceptedReceipt, OutstandingGate, RecoveredFailure};

/// What a completion was decided on: the run and the verification task it
/// named, the source and acceptance revisions that were current, every
/// receipt accepted, every historical failure a later pass recovered, and
/// every gate still outstanding. Complete when nothing is outstanding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionEvidence {
    pub run_id: String,
    pub task_id: String,
    pub task_instance: String,
    pub source_revision: String,
    pub acceptance_revision: String,
    pub accepted: Vec<AcceptedReceipt>,
    pub recovered: Vec<RecoveredFailure>,
    pub outstanding: Vec<OutstandingGate>,
}

impl CompletionEvidence {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.outstanding.is_empty()
    }
}
