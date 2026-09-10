//! Observable facts about turn admission, cancellation, and residency.

use super::{SessionResidency, TurnOutcome};

/// A consistent snapshot; requesting cancellation never manufactures a stop reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionSupervisorSnapshot {
    /// Provider attempt currently in flight, including one awaiting cancellation.
    pub active_turn: Option<u64>,
    /// Accepted follow-ups waiting in FIFO order.
    pub queued_turns: usize,
    /// A stop was requested, but a terminal response has not yet arrived.
    pub cancellation_requested: bool,
    /// The state of the owned provider connection.
    pub residency: SessionResidency,
    /// Most recently observed terminal response or unconfirmed failure.
    pub last_outcome: Option<TurnOutcome>,
    /// Whether a client still observes the message stream.
    pub client_attached: bool,
}
