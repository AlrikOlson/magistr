//! Mock helpers for testing — create `AgentSession` instances without a real ACP connection.
//!
//! Tests push messages through the returned sender and consume them via `AgentSession`.
//! No trait indirection — the session uses a channel-only backend internally.

use tokio::sync::mpsc;

use super::message::AgentMessage;
use super::session::AgentSession;
use crate::error::Result;

// Re-export for backwards compatibility (tests use `super::*`)
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
pub fn mock_session() -> (AgentSession, mpsc::UnboundedSender<Result<AgentMessage>>) {
    AgentSession::from_channel()
}

#[cfg(test)]
#[path = "mock.tests.rs"]
mod tests;
