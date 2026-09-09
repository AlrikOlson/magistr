use magistr_domain::identity::{ProviderSessionBinding, SessionId, TurnId};

/// What a session adapter has established, without equating submission with completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionOutcome {
    /// Creation or restoration completed; the provider conversation is explicitly bound.
    SessionReady { binding: ProviderSessionBinding },
    /// The provider accepted a prompt. No successful turn or chunk is certified.
    TurnAccepted { turn: TurnId },
    /// Cancellation was requested. A terminal observation is still required before reuse.
    CancellationRequested { turn: TurnId },
    /// Session termination and resource release were observed by the adapter.
    SessionClosed { session: SessionId },
}
