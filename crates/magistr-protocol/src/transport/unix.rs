//! Unix domain socket transport — JSON-RPC over `UnixStream` with
//! content-length framing.
//!
//! Provides both client-side connection (`UnixSocketTransport::connect`) and
//! server-side acceptance (`UnixSocketListener`). Messages use the same
//! LSP-style `Content-Length` framing as [`super::stdio::StdioTransport`].
//!
//! # Platform support
//!
//! Unix domain sockets are available on Linux, macOS, and other Unix-like
//! systems. This module is gated behind `#[cfg(unix)]`.

use std::path::{Path, PathBuf};

use tokio::io::BufReader;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};

use super::Transport;
use super::error::TransportError;
use super::framing;
use crate::jsonrpc::Message;

/// A transport that communicates over a Unix domain socket using
/// content-length framing.
///
/// Created either by connecting to a socket path (client-side) or from
/// an already-connected [`UnixStream`] (server-side, after accept).
pub struct UnixSocketTransport {
    reader: Option<BufReader<OwnedReadHalf>>,
    writer: Option<OwnedWriteHalf>,
    connected: bool,
}

impl std::fmt::Debug for UnixSocketTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixSocketTransport")
            .field("connected", &self.connected)
            .finish_non_exhaustive()
    }
}

impl UnixSocketTransport {
    /// Create a transport from an already-connected [`UnixStream`].
    ///
    /// Used on the server side after [`UnixSocketListener::accept`] returns
    /// a stream. The transport starts connected and ready for communication.
    #[must_use]
    pub fn from_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: Some(BufReader::new(read_half)),
            writer: Some(write_half),
            connected: true,
        }
    }

    /// Connect to a Unix domain socket at the given path.
    ///
    /// Returns a connected transport ready for sending and receiving.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the connection fails (e.g., socket
    /// does not exist or permission denied).
    pub async fn connect_to(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let stream = UnixStream::connect(path.as_ref()).await?;
        Ok(Self::from_stream(stream))
    }
}

impl Transport for UnixSocketTransport {
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

/// A server-side listener that accepts Unix domain socket connections.
///
/// Binds to a filesystem path and yields [`UnixSocketTransport`] instances
/// for each incoming connection. Removes the socket file on drop.
pub struct UnixSocketListener {
    listener: UnixListener,
    path: PathBuf,
}

impl UnixSocketListener {
    /// Bind a new listener to the given socket path.
    ///
    /// If a stale socket file exists at the path, it is removed before
    /// binding. The parent directory must already exist.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if binding fails.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let path = path.as_ref().to_path_buf();

        // Remove stale socket file if it exists.
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        let listener = UnixListener::bind(&path)?;
        Ok(Self { listener, path })
    }

    /// Accept the next incoming connection.
    ///
    /// Returns a connected [`UnixSocketTransport`] ready for communication.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the accept call fails.
    pub async fn accept(&self) -> Result<UnixSocketTransport, TransportError> {
        let (stream, _addr) = self.listener.accept().await?;
        Ok(UnixSocketTransport::from_stream(stream))
    }

    /// Returns the path this listener is bound to.
    #[must_use]
    pub fn local_path(&self) -> &Path {
        &self.path
    }
}

impl Drop for UnixSocketListener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "unix.tests.rs"]
mod tests;
