//! Mock helpers for testing — create `AgentSession` instances without a real ACP connection.
//!
//! Tests push messages through the returned sender and consume them via `AgentSession`.
//! No trait indirection — the session uses a channel-only backend internally.

use super::message::AgentMessage;
use super::observation::ObservationSender;
use super::session::AgentSession;
use crate::error::Result;

// Test helpers currently imported through `super::*`.
pub use super::mock_followups::mock_session_with_followups;

/// Create a mock session and a sender for pushing messages.
///
/// # Usage
///
/// ```ignore
/// let (mut session, tx) = mock_session();
/// tx.send(Ok(some_message)).unwrap();
/// let msg = session.next_message().await;
/// ```
pub fn mock_session() -> (AgentSession, ObservationSender<Result<AgentMessage>>) {
    AgentSession::from_channel()
}

#[cfg(test)]
#[path = "mock.tests.rs"]
mod tests;
