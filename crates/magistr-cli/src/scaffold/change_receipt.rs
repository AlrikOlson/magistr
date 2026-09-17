use super::{ApplyOutcome, Expected, FileAction, Ownership, PlannedChange};

/// One planned change and what applying it did.
#[derive(Debug, Clone)]
pub struct ChangeReceipt {
    /// Path relative to the project root.
    pub path: String,
    /// What the plan meant to do.
    pub action: FileAction,
    /// Who owned the bytes when the plan was computed.
    pub ownership: Ownership,
    /// The state the plan expected at the path.
    pub expected: Expected,
    /// What happened.
    pub outcome: ApplyOutcome,
}

impl ChangeReceipt {
    /// The receipt for `change` with `outcome`.
    pub(crate) fn new(change: &PlannedChange, outcome: ApplyOutcome) -> Self {
        Self {
            path: change.path.clone(),
            action: change.action.clone(),
            ownership: change.ownership,
            expected: change.expected(),
            outcome,
        }
    }

    /// One line for a log or a receipt listing: the path, the outcome and
    /// its detail when there is one.
    #[must_use]
    pub fn describe(&self) -> String {
        match self.outcome.detail() {
            Some(detail) => format!("{}: {} ({detail})", self.path, self.outcome.label()),
            None => format!("{}: {}", self.path, self.outcome.label()),
        }
    }
}
