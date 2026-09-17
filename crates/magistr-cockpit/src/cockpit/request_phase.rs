//! Admission and outcome of a roadmap request.

/// A request's state, independently of an implementation turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestPhase {
    /// Submitted locally, awaiting the command owner.
    Waiting,
    /// The current work must reach a safe boundary first.
    Queued,
    /// The request has its own agent session.
    Working,
    /// The agent ended its response normally.
    Complete,
    /// The user cancelled, or the agent ended without completing.
    Stopped,
    /// Delivery or execution failed.
    Failed,
}

impl RequestPhase {
    /// Whether this request can still produce an answer.
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Waiting | Self::Queued | Self::Working)
    }

    /// Plain language for the request's current state.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Waiting => "Waiting for response",
            Self::Queued => "Queued — after the current work finishes",
            Self::Working => "Updating roadmap",
            Self::Complete => "Response complete",
            Self::Stopped => "Stopped",
            Self::Failed => "Could not finish",
        }
    }
}
