use super::Peer;

/// Sticky stop requiring the host to save a checkpoint. No variant authorizes work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationStop {
    Deadline,
    TokensExhausted,
    /// Observed starts exceeded the ceiling (reached it in legacy v1 journals).
    AttemptsExhausted,
    RoundsExhausted,
    PeerUnavailable(Peer),
}
