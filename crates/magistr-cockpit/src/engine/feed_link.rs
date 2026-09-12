//! What the live feed tells the re-seed, without going through the model.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

/// The handle the feed relay and the re-seed share: whether the feed
/// holds the broadcast socket right now, and a request for the store to
/// be read again.
///
/// The bit keeps the re-seed out of the way while frames arrive on their
/// own. The request is how the feed asks for a snapshot when frames may
/// have been missed — on every attachment (frames emitted before it
/// registered are gone), on a gap in the publisher's sequence, on a new
/// owner — so the model never has to: it stays pure, and the observer
/// cockpit, which takes no commands, is covered the same way. A request
/// is a permit, not a wakeup: one made while nobody waits is kept for the
/// next wait, and several collapse into one read.
#[derive(Clone, Debug, Default)]
pub struct FeedLink(Arc<Inner>);

#[derive(Debug, Default)]
struct Inner {
    attached: AtomicBool,
    snapshot: Notify,
}

impl FeedLink {
    /// A link reading detached, with nothing requested.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Say whether the feed holds the socket right now.
    pub fn set(&self, attached: bool) {
        self.0.attached.store(attached, Ordering::Relaxed);
    }

    /// Whether it does.
    #[must_use]
    pub fn is_attached(&self) -> bool {
        self.0.attached.load(Ordering::Relaxed)
    }

    /// Ask for the store to be read again.
    pub fn request_snapshot(&self) {
        self.0.snapshot.notify_one();
    }

    /// Wait for the next request (or take one already made).
    pub async fn snapshot_requested(&self) {
        self.0.snapshot.notified().await;
    }
}

#[cfg(test)]
#[path = "feed_link.tests.rs"]
mod tests;
