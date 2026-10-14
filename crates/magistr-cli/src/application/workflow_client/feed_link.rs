//! Coalesced requests to validate workflow owners after changes or missed frames.

use std::sync::Arc;
use tokio::sync::Notify;

/// A permit survives until a reader takes it; several requests collapse into one read.
#[derive(Clone, Debug, Default)]
pub(crate) struct FeedLink(Arc<Notify>);

impl FeedLink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_snapshot(&self) {
        self.0.notify_one();
    }

    pub async fn snapshot_requested(&self) {
        self.0.notified().await;
    }
}

#[cfg(test)]
#[path = "feed_link.tests.rs"]
mod tests;
