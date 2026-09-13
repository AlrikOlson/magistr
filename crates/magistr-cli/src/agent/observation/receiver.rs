//! The observer's end of an observation channel.

use std::sync::Arc;

use tokio::sync::mpsc::error::TryRecvError;

use super::observed::Observed;
use super::queue::Shared;

/// Takes items in the channel's order. Dropping or closing it ends the
/// owner's ability to send; the owner reads that as a detached observer.
pub struct ObservationReceiver<T> {
    shared: Arc<Shared<T>>,
}

impl<T: Observed> ObservationReceiver<T> {
    pub(super) fn new(shared: Arc<Shared<T>>) -> Self {
        Self { shared }
    }

    /// The next item; `None` once the channel is closed and drained.
    /// Dropping a pending call loses nothing.
    pub async fn recv(&mut self) -> Option<T> {
        loop {
            // Register interest before looking, so a send between the look
            // and the wait still wakes this call.
            let notified = self.shared.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            match self.shared.take() {
                (Some(item), _) => return Some(item),
                (None, true) => return None,
                (None, false) => notified.await,
            }
        }
    }

    /// The next item already pending, without waiting: `Empty` when
    /// nothing is pending yet, `Disconnected` once the channel is closed
    /// and drained.
    ///
    /// # Errors
    /// Nothing pending, or nothing ever again.
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        match self.shared.take() {
            (Some(item), _) => Ok(item),
            (None, true) => Err(TryRecvError::Disconnected),
            (None, false) => Err(TryRecvError::Empty),
        }
    }

    /// Stop observing: nothing more is admitted and what was pending is
    /// discarded.
    pub fn close(&mut self) {
        self.shared.close();
    }

    /// Items waiting, lag markers included.
    #[must_use]
    pub fn pending_items(&self) -> usize {
        self.shared.pending_items()
    }

    /// Coalesced text bytes waiting.
    #[must_use]
    pub fn pending_text_bytes(&self) -> usize {
        self.shared.pending_text_bytes()
    }

    /// Text bytes trimmed behind lag markers over the channel's life.
    #[must_use]
    pub fn trimmed_bytes(&self) -> usize {
        self.shared.trimmed_bytes()
    }

    /// Whether the lossless bound overflowed and observation ended behind
    /// its marker.
    #[must_use]
    pub fn has_overflowed(&self) -> bool {
        self.shared.has_overflowed()
    }
}

impl<T> Drop for ObservationReceiver<T> {
    fn drop(&mut self) {
        self.shared.close();
    }
}

impl<T> std::fmt::Debug for ObservationReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObservationReceiver")
            .field("pending_items", &self.shared.pending_items())
            .field("pending_text_bytes", &self.shared.pending_text_bytes())
            .finish()
    }
}
