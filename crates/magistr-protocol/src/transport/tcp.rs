//! TCP transport — JSON-RPC over `TcpStream` with content-length framing.
//!
//! Provides both client-side connection (`TcpTransport::connect_to` with
//! configurable timeout) and server-side acceptance (`TcpTransportListener`).
//! Messages use the same LSP-style `Content-Length` framing as all other
//! transports in this crate.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

use super::Transport;
use super::error::TransportError;
use super::framing;
use crate::jsonrpc::Message;

/// A transport that communicates over a TCP socket using content-length
/// framing.
///
/// Created either by connecting to a remote address (client-side) or from
/// an already-connected [`TcpStream`] (server-side, after accept).
pub struct TcpTransport {
    reader: Option<BufReader<OwnedReadHalf>>,
    writer: Option<OwnedWriteHalf>,
    connected: bool,
}

impl std::fmt::Debug for TcpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpTransport")
            .field("connected", &self.connected)
            .finish_non_exhaustive()
    }
}

impl TcpTransport {
    /// Create a transport from an already-connected [`TcpStream`].
    ///
    /// Used on the server side after [`TcpTransportListener::accept`] returns
    /// a stream. The transport starts connected and ready for communication.
    #[must_use]
    pub fn from_stream(stream: TcpStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: Some(BufReader::new(read_half)),
            writer: Some(write_half),
            connected: true,
        }
    }

    /// Connect to a TCP address with a timeout.
    ///
    /// Returns a connected transport ready for sending and receiving.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionTimeout`] if the connection is not
    /// established within the given duration.
    /// Returns [`TransportError::Io`] if the connection fails for other reasons.
    pub async fn connect_to(addr: SocketAddr, timeout: Duration) -> Result<Self, TransportError> {
        let stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| TransportError::ConnectionTimeout)?
            .map_err(TransportError::Io)?;

        // Disable Nagle's algorithm for low-latency message exchange.
        stream.set_nodelay(true).ok();

        Ok(Self::from_stream(stream))
    }
}

impl Transport for TcpTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        self.reader = None;
        self.writer = None;
        Ok(())
    }

    async fn send_message(&mut self, msg: &Message) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        let writer = self.writer.as_mut().ok_or(TransportError::NotConnected)?;

        let json = match msg {
            Message::Request(req) => serde_json::to_vec(req)?,
            Message::Response(resp) => serde_json::to_vec(resp)?,
            Message::Error(err) => serde_json::to_vec(err)?,
            Message::Notification(notif) => serde_json::to_vec(notif)?,
        };

        framing::write_frame(writer, &json).await
    }

    async fn recv_message(&mut self) -> Result<Message, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        let reader = self.reader.as_mut().ok_or(TransportError::NotConnected)?;

        let payload = framing::read_frame(reader).await?;
        let value: serde_json::Value = serde_json::from_slice(&payload)?;
        Message::from_value(value).map_err(|e| TransportError::InvalidFrame(e.to_string()))
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

/// A server-side listener that accepts TCP connections.
///
/// Binds to a socket address and yields [`TcpTransport`] instances for
/// each incoming connection.
pub struct TcpTransportListener {
    listener: TcpListener,
}

impl TcpTransportListener {
    /// Bind a new listener to the given address.
    ///
    /// Use `"127.0.0.1:0"` to let the OS assign an available port (useful
    /// for tests). Query the assigned address with [`local_addr`](Self::local_addr).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if binding fails.
    pub async fn bind(addr: impl ToSocketAddrs) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }

    /// Accept the next incoming connection.
    ///
    /// Returns a connected [`TcpTransport`] ready for communication.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the accept call fails.
    pub async fn accept(&self) -> Result<TcpTransport, TransportError> {
        let (stream, _addr) = self.listener.accept().await?;
        stream.set_nodelay(true).ok();
        Ok(TcpTransport::from_stream(stream))
    }

    /// Returns the local address this listener is bound to.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the address cannot be determined.
    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.listener.local_addr().map_err(TransportError::Io)
    }
}

#[cfg(test)]
#[path = "tcp.tests.rs"]
mod tests;
