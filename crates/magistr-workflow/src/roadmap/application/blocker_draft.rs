use crate::roadmap::domain::BlockerKind;

/// A blocker requiring native validation and a workflow timestamp.
#[derive(Debug, Clone)]
pub struct BlockerDraft {
    pub kind: BlockerKind,
    pub reason: String,
    pub evidence: Option<String>,
}
