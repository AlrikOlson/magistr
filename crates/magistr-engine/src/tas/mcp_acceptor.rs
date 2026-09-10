//! How an MCP owner takes an attached MCP session.

use std::sync::Arc;

/// The connection an acceptor is handed: the client's socket on Unix,
/// nothing anywhere else (there is no endpoint to accept on).
#[cfg(unix)]
type Attached = tokio::net::UnixStream;
#[cfg(not(unix))]
type Attached = ();

/// What an [`OwnerEndpoint`](super::OwnerEndpoint) hands an attached
/// connection to after a welcomed [`AttachKind::Mcp`](super::AttachKind::Mcp)
/// handshake: the host's own rmcp session, served over the socket the
/// client is holding. Supplied by the host that has an MCP service to
/// offer (`magistr mcp-serve`); a host without one refuses MCP attachment
/// at the handshake. The engine crate never sees MCP itself.
pub type McpAcceptor = Arc<dyn Fn(Attached) + Send + Sync>;
