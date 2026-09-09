//! A prompt waiting for its session's worker.

use agent_client_protocol::Responder;
use agent_client_protocol::schema::v1::{PromptRequest, PromptResponse};
use tokio_util::sync::CancellationToken;

/// One `session/prompt` in a session's queue, with the responder that
/// answers it and the cancellation that was live when it was queued — so a
/// `session/cancel` that arrives before the worker reaches this turn has
/// already cancelled it.
pub(crate) struct QueuedTurn {
    pub request: PromptRequest,
    pub responder: Responder<PromptResponse>,
    pub cancel: CancellationToken,
}
