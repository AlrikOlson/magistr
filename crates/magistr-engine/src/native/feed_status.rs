//! Whether a subscriber is attached to the project's broadcast socket.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The state of a subscriber's connection to `magistr mcp-serve`'s broadcast
/// socket, relayed to the TUI as the `native.feed` system message (the
/// payload also carries the socket path under `socket`).
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
    /// The system-message subtype the status travels under.
    pub const SUBTYPE: &'static str = "native.feed";

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

    /// Read a status back out of a `native.feed` payload.
    #[must_use]
    pub fn from_value(value: &Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }

    /// The `native.feed` payload: the status plus the socket it concerns.
    #[must_use]
    pub fn to_payload(self, socket: &Path) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "socket".to_owned(),
                Value::String(socket.display().to_string()),
            );
        }
        value
    }
}

#[cfg(test)]
#[path = "feed_status.tests.rs"]
mod tests;
