//! Transport abstraction for magistr protocol communication.
//!
//! The [`Transport`] trait defines the async interface for sending and
//! receiving JSON-RPC messages over any underlying channel (stdio, Unix
//! socket, TCP, WebSocket). Each transport handles its own framing and
//! connection lifecycle.
//!
//! # Modules
//!
//! - [`error`] — Transport error types
//! - [`framing`] — Content-length frame encoding/decoding
//! - [`stdio`] — Stdio transport implementation
//! - [`tcp`] — TCP transport implementation
//! - [`unix`] — Unix domain socket transport (Unix only; Windows attaches over TCP)
//! - [`socket_path`] — Socket path conventions
//! - [`ws`] — WebSocket transport with ping/pong keepalive

pub mod error;
pub mod framing;
pub mod socket_path;
pub mod stdio;
pub mod tcp;
#[cfg(unix)]
pub mod unix;
pub mod ws;

pub use error::TransportError;
pub use socket_path::{default_socket_dir, ensure_socket_dir, server_socket_path};
pub use stdio::StdioTransport;
pub use tcp::{TcpTransport, TcpTransportListener};
#[cfg(unix)]
pub use unix::{UnixSocketListener, UnixSocketTransport};
pub use ws::{WebSocketListener, WebSocketTransport};

use crate::jsonrpc::Message;

/// Async transport for sending and receiving JSON-RPC messages.
///
/// Implementors handle connection lifecycle and framing. The trait is
/// object-safe and `Send + Sync` so transports can be used from any
/// async task.
///
/// # Connection lifecycle
///
/// 1. Call [`connect`](Transport::connect) to establish the connection.
/// 2. Use [`send_message`](Transport::send_message) and
///    [`recv_message`](Transport::recv_message) for communication.
/// 3. Call [`disconnect`](Transport::disconnect) to cleanly shut down.
///
/// Reconnecting is done by calling `disconnect()` followed by `connect()`.
pub trait Transport: Send + Sync {
    /// Establish the transport connection.
    ///
    /// For transports like stdio this is a no-op that marks the transport
    /// as ready. For socket transports this performs the actual connection.
    fn connect(&mut self) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;

    /// Cleanly shut down the transport connection.
    ///
    /// After disconnecting, [`send_message`](Transport::send_message) and
    /// [`recv_message`](Transport::recv_message) should return
    /// [`TransportError::NotConnected`].
    fn disconnect(
        &mut self,
    ) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;

    /// Send a JSON-RPC message over the transport.
    ///
    /// The message is serialized and framed according to the transport's
    /// framing protocol before being written.
    fn send_message(
        &mut self,
        msg: &Message,
    ) -> impl std::future::Future<Output = Result<(), TransportError>> + Send;

    /// Receive the next JSON-RPC message from the transport.
    ///
    /// Blocks until a complete framed message is available or an error
    /// occurs. Returns [`TransportError::ConnectionClosed`] when the
    /// remote end closes the connection.
    fn recv_message(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Message, TransportError>> + Send;

    /// Check whether the transport is currently connected.
    fn is_connected(&self) -> bool;
}
