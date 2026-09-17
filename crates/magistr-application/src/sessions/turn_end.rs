//! Why a turn ended.

/// The agent's stop reason for a `session/prompt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TurnEnd {
    /// The agent finished on its own.
    EndTurn,
    /// The model's token limit was hit.
    MaxTokens,
    /// The agent's turn-request limit was hit.
    MaxTurnRequests,
    /// The model refused.
    Refusal,
    /// The client cancelled.
    Cancelled,
    /// A stop reason this cockpit does not know.
    Other,
}
