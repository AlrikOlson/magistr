//! magistr-protocol — JSON-RPC 2.0 protocol schema for the magistr engine.
//!
//! This crate defines the wire-format types for communicating with a magistr
//! engine server. It is transport-agnostic — any transport (stdio, Unix
//! socket, TCP, WebSocket) serializes/deserializes these types.
//!
//! # Modules
//!
//! - [`jsonrpc`] — JSON-RPC 2.0 base types (request, response, notification)
//! - [`methods`] — Typed method params and results for all engine domains
//! - [`notification`] — Server-to-client event notifications
//! - [`error`] — Protocol error codes and types
//!
//! # Protocol version
//!
//! The protocol follows semver. Clients should check [`PROTOCOL_VERSION`]
//! on connect and verify compatibility. New methods may be added in minor
//! versions; breaking changes require a major version bump.
//!
//! Zero TUI dependencies. No ratatui, crossterm, or tachyonfx.

pub mod client;
pub mod error;
pub mod jsonrpc;
pub mod methods;
pub mod notification;
pub mod transport;

// ── Protocol version ────────────────────────────────────────────────────────

/// Current protocol version (semver).
///
/// - Major: breaking changes to existing methods
/// - Minor: new methods, new notification types
/// - Patch: documentation, bug fixes in type definitions
pub const PROTOCOL_VERSION: &str = "0.1.0";

/// Protocol version components for programmatic comparison.
pub const PROTOCOL_VERSION_MAJOR: u32 = 0;
pub const PROTOCOL_VERSION_MINOR: u32 = 1;
pub const PROTOCOL_VERSION_PATCH: u32 = 0;

/// Check if a client's protocol version is compatible with this server.
///
/// Compatibility rule: same major version, client minor <= server minor.
/// During 0.x development, minor versions are treated as breaking.
#[must_use]
pub fn is_compatible(client_major: u32, client_minor: u32) -> bool {
    if PROTOCOL_VERSION_MAJOR == 0 {
        // During 0.x, require exact minor match
        client_major == PROTOCOL_VERSION_MAJOR && client_minor == PROTOCOL_VERSION_MINOR
    } else {
        client_major == PROTOCOL_VERSION_MAJOR && client_minor <= PROTOCOL_VERSION_MINOR
    }
}

// Re-export primary types for convenience.
pub use error::ProtocolError;
pub use jsonrpc::{ErrorResponse, Message, Notification, Request, RequestId, Response};
pub use methods::method;
pub use notification::{EventNotification, engine_event_to_notification};
pub use transport::{
    StdioTransport, TcpTransport, TcpTransportListener, Transport, TransportError,
    WebSocketListener, WebSocketTransport, default_socket_dir, ensure_socket_dir,
    server_socket_path,
};
#[cfg(unix)]
pub use transport::{UnixSocketListener, UnixSocketTransport};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_version_format() {
        let expected = format!(
            "{}.{}.{}",
            PROTOCOL_VERSION_MAJOR, PROTOCOL_VERSION_MINOR, PROTOCOL_VERSION_PATCH
        );
        assert_eq!(PROTOCOL_VERSION, expected);
    }

    #[test]
    fn compatible_exact_match() {
        assert!(is_compatible(0, 1));
    }

    #[test]
    fn incompatible_different_minor_during_0x() {
        assert!(!is_compatible(0, 0));
        assert!(!is_compatible(0, 2));
    }

    #[test]
    fn incompatible_different_major() {
        assert!(!is_compatible(1, 0));
        assert!(!is_compatible(2, 1));
    }
}
