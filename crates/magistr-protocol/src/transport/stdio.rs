//! Stdio transport — JSON-RPC over stdin/stdout with content-length framing.
//!
//! This transport is the primary communication channel for the magistr CLI,
//! where the server process reads from stdin and writes to stdout.
//!
//! The generic type parameters allow testing with mock readers/writers
//! while production code uses `tokio::io::Stdin` / `tokio::io::Stdout`.

use tokio::io::{AsyncRead, AsyncWrite, BufReader};

use super::Transport;
use super::error::TransportError;
use super::framing;
use crate::jsonrpc::Message;

/// A transport that communicates over async reader/writer pairs using
/// content-length framing.
///
/// Generic over reader (`R`) and writer (`W`) so tests can substitute
/// in-memory buffers while production code uses real stdin/stdout.
///
/// Internally wraps the reader in a `BufReader` to support the
/// line-oriented header parsing required by content-length framing.
pub struct StdioTransport<R, W> {
    reader: BufReader<R>,
    writer: W,
    connected: bool,
}

impl<R, W> StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    /// Create a new stdio transport wrapping the given reader and writer.
    ///
    /// The transport starts in a disconnected state. Call
    /// [`connect`](Transport::connect) before sending or receiving.
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer,
            connected: false,
        }
    }

    /// Consume the transport and return the inner writer.
    ///
    /// Useful in tests to inspect what was written.
    pub fn into_writer(self) -> W {
        self.writer
    }
}

impl<R, W> Transport for StdioTransport<R, W>
where
    R: AsyncRead + Unpin + Send + Sync,
    W: AsyncWrite + Unpin + Send + Sync,
{
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        Ok(())
    }

    async fn send_message(&mut self, msg: &Message) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        let json = match msg {
            Message::Request(req) => serde_json::to_vec(req)?,
            Message::Response(resp) => serde_json::to_vec(resp)?,
            Message::Error(err) => serde_json::to_vec(err)?,
            Message::Notification(notif) => serde_json::to_vec(notif)?,
        };

        framing::write_frame(&mut self.writer, &json).await
    }

    async fn recv_message(&mut self) -> Result<Message, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        let payload = framing::read_frame(&mut self.reader).await?;
        let value: serde_json::Value = serde_json::from_slice(&payload)?;
        Message::from_value(value).map_err(|e| TransportError::InvalidFrame(e.to_string()))
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
#[path = "stdio.tests.rs"]
mod tests;
