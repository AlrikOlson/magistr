//! Whether the live feed currently holds the broadcast socket.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A one-bit handle shared between the feed relay and the periodic
/// re-seed: the feed sets it as it attaches to and loses the broadcast
/// socket, and the re-seed reads it to stay out of the way while frames
/// are arriving on their own.
///
/// Deliberately not a channel: nobody waits on the change, and the
/// re-seed only ever asks the question at the top of its own interval.
#[derive(Clone, Debug, Default)]
pub struct FeedAttached(Arc<AtomicBool>);

impl FeedAttached {
    /// A handle reading detached.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Say whether the feed holds the socket right now.
    pub fn set(&self, attached: bool) {
        self.0.store(attached, Ordering::Relaxed);
    }

    /// Whether it does.
    #[must_use]
    pub fn is_attached(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
#[path = "attached.tests.rs"]
mod tests;
