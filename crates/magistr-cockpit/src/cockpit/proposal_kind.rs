//! Which field of a task an open proposal wants to change.

use magistr_workflow_contracts::Task;

/// The kind of an open plan proposal on a task, mirroring the three
/// the engine records: re-prioritization, status, title.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProposalKind {
    /// A re-prioritization proposal.
    Reprioritize,
    /// A status proposal (from a tracker sweep or an agent).
    Status,
    /// A title proposal.
    Title,
}

impl ProposalKind {
    /// Every kind, in the order the engine's task record carries them.
    pub const ALL: [Self; 3] = [Self::Reprioritize, Self::Status, Self::Title];

    /// The open proposal of this kind on `task`, as the card shows it:
    /// the suggested value in words and the reason the proposer gave.
    #[must_use]
    pub fn on(self, task: &Task) -> Option<(String, String)> {
        match self {
            Self::Reprioritize => task
                .reprioritize
                .as_ref()
                .map(|p| (p.suggested_priority.to_string(), p.reason.clone())),
            Self::Status => task
                .status_proposal
                .as_ref()
                .map(|p| (p.suggested_status.as_wire().to_owned(), p.reason.clone())),
            Self::Title => task
                .title_proposal
                .as_ref()
                .map(|p| (p.suggested_title.clone(), p.reason.clone())),
        }
    }

    /// The field's name as the card says it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Reprioritize => "priority",
            Self::Status => "status",
            Self::Title => "title",
        }
    }
}
