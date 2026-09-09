//! WebSocket transport — JSON-RPC over WebSocket with native message framing.
//!
//! Provides both client-side connection (`WebSocketTransport::connect_to` with
//! configurable timeout) and server-side acceptance (`WebSocketListener`).
//! Messages are sent as WebSocket Text frames containing JSON. Unlike other
//! transports in this crate, WebSocket has its own message boundaries so no
//! content-length framing is needed.
//!
//! Ping/pong keepalive is supported via [`WebSocketTransport::send_ping`].
//! Tungstenite automatically replies to incoming Ping frames with Pong.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, accept_async, connect_async};

use super::Transport;
use super::error::TransportError;
use crate::jsonrpc::Message;

/// Split halves of a WebSocket stream for independent send/recv.
type WsSink = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, WsMessage>;
type WsStream = futures_util::stream::SplitStream<WebSocketStream<TcpStream>>;

/// Split halves for a client connection (which may use TLS).
type ClientWsSink =
    futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>;
type ClientWsStream = futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// A transport that communicates over WebSocket using JSON text frames.
///
/// Created either by connecting to a URL (client-side) or from an accepted
/// WebSocket connection (server-side, after upgrade).
pub struct WebSocketTransport {
    sink: Option<SinkVariant>,
    stream: Option<StreamVariant>,
    connected: bool,
}

/// Sink variant to handle both server (plain TCP) and client (maybe TLS) streams.
enum SinkVariant {
    Server(WsSink),
    Client(ClientWsSink),
}

/// Stream variant to handle both server (plain TCP) and client (maybe TLS) streams.
enum StreamVariant {
    Server(WsStream),
    Client(ClientWsStream),
}

impl std::fmt::Debug for WebSocketTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketTransport")
            .field("connected", &self.connected)
            .finish_non_exhaustive()
    }
}

impl WebSocketTransport {
    /// Create a transport from an already-upgraded server-side WebSocket stream.
    ///
    /// Used after [`WebSocketListener::accept`] upgrades a TCP connection.
    /// The transport starts connected and ready for communication.
    #[must_use]
    pub fn from_server_stream(ws_stream: WebSocketStream<TcpStream>) -> Self {
        let (sink, stream) = ws_stream.split();
        Self {
            sink: Some(SinkVariant::Server(sink)),
            stream: Some(StreamVariant::Server(stream)),
            connected: true,
        }
    }

    /// Connect to a WebSocket server at the given URL with a timeout.
    ///
    /// The URL should use the `ws://` scheme (e.g., `ws://127.0.0.1:8080`).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ConnectionTimeout`] if the connection is not
    /// established within the given duration.
    /// Returns [`TransportError::WebSocket`] if the WebSocket handshake fails.
    pub async fn connect_to(url: &str, timeout: Duration) -> Result<Self, TransportError> {
        let (ws_stream, _response) = tokio::time::timeout(timeout, connect_async(url))
            .await
            .map_err(|_| TransportError::ConnectionTimeout)?
            .map_err(|e| TransportError::WebSocket(e.to_string()))?;

        let (sink, stream) = ws_stream.split();
        Ok(Self {
            sink: Some(SinkVariant::Client(sink)),
            stream: Some(StreamVariant::Client(stream)),
            connected: true,
        })
    }

    /// Send a WebSocket Ping frame.
    ///
    /// Tungstenite on the remote side will automatically reply with a Pong.
    /// This can be used for keepalive detection.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::NotConnected`] if the transport is disconnected.
    pub async fn send_ping(&mut self) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        let ping = WsMessage::Ping(Vec::new().into());
        match self.sink.as_mut().ok_or(TransportError::NotConnected)? {
            SinkVariant::Server(sink) => sink
                .send(ping)
                .await
                .map_err(|e| TransportError::WebSocket(e.to_string())),
            SinkVariant::Client(sink) => sink
                .send(ping)
                .await
                .map_err(|e| TransportError::WebSocket(e.to_string())),
        }
    }
}

impl Transport for WebSocketTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        // Send a close frame if we still have a sink.
        if let Some(sink) = &mut self.sink {
            let close = WsMessage::Close(None);
            let _ = match sink {
                SinkVariant::Server(s) => s.send(close).await,
                SinkVariant::Client(s) => s.send(close).await,
            };
        }
        self.connected = false;
        self.sink = None;
        self.stream = None;
        Ok(())
    }

    async fn send_message(&mut self, msg: &Message) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        let json = match msg {
            Message::Request(req) => serde_json::to_string(req)?,
            Message::Response(resp) => serde_json::to_string(resp)?,
            Message::Error(err) => serde_json::to_string(err)?,
            Message::Notification(notif) => serde_json::to_string(notif)?,
        };

        let ws_msg = WsMessage::Text(json.into());
        match self.sink.as_mut().ok_or(TransportError::NotConnected)? {
            SinkVariant::Server(sink) => sink
                .send(ws_msg)
                .await
                .map_err(|e| TransportError::WebSocket(e.to_string())),
            SinkVariant::Client(sink) => sink
                .send(ws_msg)
                .await
                .map_err(|e| TransportError::WebSocket(e.to_string())),
        }
    }

    async fn recv_message(&mut self) -> Result<Message, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        loop {
            let ws_msg = match self.stream.as_mut().ok_or(TransportError::NotConnected)? {
                StreamVariant::Server(stream) => stream.next().await,
                StreamVariant::Client(stream) => stream.next().await,
            };

            match ws_msg {
                Some(Ok(WsMessage::Text(text))) => {
                    let value: serde_json::Value = serde_json::from_str(&text)?;
                    return Message::from_value(value)
                        .map_err(|e| TransportError::InvalidFrame(e.to_string()));
                }
                Some(Ok(WsMessage::Close(_))) => {
                    self.connected = false;
                    return Err(TransportError::ConnectionClosed);
                }
                // Skip ping/pong/binary — tungstenite handles pong auto-reply
                Some(Ok(_)) => continue,
                Some(Err(e)) => {
                    self.connected = false;
                    return Err(TransportError::WebSocket(e.to_string()));
                }
                None => {
                    self.connected = false;
                    return Err(TransportError::ConnectionClosed);
                }
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

/// A server-side listener that accepts TCP connections and upgrades them
/// to WebSocket.
///
/// Binds to a socket address and yields [`WebSocketTransport`] instances
/// for each incoming connection after performing the WebSocket handshake.
pub struct WebSocketListener {
    listener: TcpListener,
}

impl WebSocketListener {
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

    /// Accept the next incoming connection and upgrade it to WebSocket.
    ///
    /// Returns a connected [`WebSocketTransport`] ready for communication.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Io`] if the TCP accept fails.
    /// Returns [`TransportError::WebSocket`] if the WebSocket handshake fails.
    pub async fn accept(&self) -> Result<WebSocketTransport, TransportError> {
        let (tcp_stream, _addr) = self.listener.accept().await?;
        tcp_stream.set_nodelay(true).ok();
        let ws_stream = accept_async(tcp_stream)
            .await
            .map_err(|e| TransportError::WebSocket(e.to_string()))?;
        Ok(WebSocketTransport::from_server_stream(ws_stream))
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
#[path = "ws.tests.rs"]
mod tests;
