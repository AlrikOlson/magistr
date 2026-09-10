//! Evidence for the most recently ended provider attempt.

use agent_client_protocol::schema::v1::StopReason;

/// A confirmed response retains its stop reason; missing confirmation retains an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnOutcome {
    /// Monotonic session-local attempt identifier.
    pub turn_id: u64,
    /// Present only when an actual ACP prompt response was received.
    pub stop_reason: Option<StopReason>,
    /// Transport failure or termination before a terminal response.
    pub error: Option<String>,
}
