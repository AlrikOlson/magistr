//! Which field of a chunk an open proposal wants to change.

/// The kind of an open roadmap proposal on a chunk, mirroring the three
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
    /// Every kind, in the order the engine's chunk payload carries them.
    pub const ALL: [Self; 3] = [Self::Reprioritize, Self::Status, Self::Title];

    /// The proposal's key on a `chunk_changed` frame's chunk payload.
    #[must_use]
    pub const fn payload_key(self) -> &'static str {
        match self {
            Self::Reprioritize => "reprioritize",
            Self::Status => "status_proposal",
            Self::Title => "title_proposal",
        }
    }

    /// The suggested value's key inside the proposal object.
    #[must_use]
    pub const fn suggestion_key(self) -> &'static str {
        match self {
            Self::Reprioritize => "suggested_priority",
            Self::Status => "suggested_status",
            Self::Title => "suggested_title",
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
