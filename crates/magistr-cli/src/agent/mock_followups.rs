//! Mock session variant that captures follow-up messages for testing.

use std::sync::{Arc, Mutex};

use super::message::AgentMessage;
use super::observation::ObservationSender;
use super::session::AgentSession;
use crate::error::Result;

/// Create a mock session that also captures follow-up messages.
///
/// Returns `(session, message_sender, follow_ups)`.
#[allow(clippy::type_complexity)]
pub fn mock_session_with_followups() -> (
    AgentSession,
    ObservationSender<Result<AgentMessage>>,
    Arc<Mutex<Vec<String>>>,
) {
    AgentSession::from_channel_with_followups()
}
