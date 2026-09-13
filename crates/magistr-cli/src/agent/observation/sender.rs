//! The owner's end of an observation channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::observed::Observed;
use super::queue::Shared;

/// Admits items under the channel's bounds. Cloned freely: every clone
/// feeds the same queue and learns of closure the same way; once the
/// last clone is gone the receiver drains what is pending and then sees
/// the end of the stream.
pub struct ObservationSender<T> {
    shared: Arc<Shared<T>>,
    /// Live senders on this channel, so the last one can say it went.
    senders: Arc<AtomicUsize>,
}

impl<T> Clone for ObservationSender<T> {
    fn clone(&self) -> Self {
        self.senders.fetch_add(1, Ordering::SeqCst);
        Self {
            shared: Arc::clone(&self.shared),
            senders: Arc::clone(&self.senders),
        }
    }
}

impl<T> Drop for ObservationSender<T> {
    fn drop(&mut self) {
        if self.senders.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.shared.producers_gone();
        }
    }
}

impl<T: Observed> ObservationSender<T> {
    pub(super) fn new(shared: Arc<Shared<T>>) -> Self {
        Self {
            shared,
            senders: Arc::new(AtomicUsize::new(1)),
        }
    }

    /// Admit `item`. `Err` hands it back once the channel is closed: the
    /// observer went away, or the lossless bound overflowed and
    /// observation ended behind its marker.
    ///
    /// # Errors
    /// The item, when the channel is closed.
    pub fn send(&self, item: T) -> Result<(), T> {
        self.shared.send(item)
    }

    /// Whether nothing more can be admitted.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.shared.is_closed()
    }
}

impl<T> std::fmt::Debug for ObservationSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObservationSender")
            .field("closed", &self.shared.is_closed())
            .finish()
    }
}
