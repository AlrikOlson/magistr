//! Whether a subscriber is attached to the project's broadcast socket.

use serde::{Deserialize, Serialize};

/// The state of a subscriber's connection to `magistr mcp-serve`'s broadcast
/// socket, as the cockpit's feed reports it on the inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FeedStatus {
    /// Not attached yet: the socket has not appeared.
    Waiting,
    /// Attached. `reconnect` is true for every connection after the first —
    /// frames emitted while the feed was down are lost, so the host should
    /// re-seed its views.
    Connected {
        /// Whether this connection follows an earlier one.
        reconnect: bool,
    },
    /// The server went away; the feed keeps retrying.
    Disconnected,
}

impl FeedStatus {
    /// Attached to a live server.
    #[must_use]
    pub fn is_connected(self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// No frames are arriving: waiting for the socket, or reconnecting.
    #[must_use]
    pub fn is_unavailable(self) -> bool {
        !self.is_connected()
    }
}

#[cfg(test)]
#[path = "feed_status.tests.rs"]
mod tests;
