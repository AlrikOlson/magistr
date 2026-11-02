use crate::plan::domain::WaitingReason;

/// A blocker requiring native validation and a workflow timestamp.
#[derive(Debug, Clone)]
pub struct WaitingDraft {
    pub kind: WaitingReason,
    pub reason: String,
    pub evidence: Option<String>,
}
