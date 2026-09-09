//! What the registry keeps per open session.

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::queued_turn::QueuedTurn;

/// An open session: the queue its worker drains, the cancellation the next
/// queued turn inherits, and the model it is on.
pub(crate) struct SessionEntry {
    pub queue: mpsc::UnboundedSender<QueuedTurn>,
    pub cancel: CancellationToken,
    pub model: Option<String>,
}
