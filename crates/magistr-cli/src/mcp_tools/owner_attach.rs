//! Workflow owner endpoint; every attached MCP session shares its one writer.
#[cfg(unix)]
use super::protocol_cap::ProtocolCap;
use super::service::MagistrService;
#[cfg(unix)]
use magistr_engine::native::McpAcceptor;
use magistr_engine::native::OwnerEndpoint;
#[cfg(unix)]
use rmcp::ServiceExt as _;
use std::io;
#[cfg(unix)]
use std::sync::Arc;

impl MagistrService {
    /// Serve this owner's endpoint: an attached `magistr mcp-serve` gets a
    /// session of this same service over the socket, under the same
    /// protocol cap the stdio session runs under. The endpoint lives as
    /// long as the handle — keep it beside the stdio session and drop it
    /// when that session ends. Attached bridges reconnect on their next
    /// request after the endpoint and lease have been released.
    ///
    /// # Errors
    /// The endpoint cannot be served: a listener is live at the socket, or
    /// it cannot be bound.
    #[cfg(unix)]
    pub async fn serve_owner_endpoint(&self) -> io::Result<Option<OwnerEndpoint>> {
        let service = self.clone();
        let acceptor: McpAcceptor = Arc::new(move |stream: tokio::net::UnixStream, cancel| {
            let service = service.clone();
            Box::pin(async move {
                let transport = ProtocolCap::new(
                    &service,
                    rmcp::transport::IntoTransport::<rmcp::service::RoleServer, io::Error, _>::into_transport(
                        stream,
                    ),
                );
                match service.serve_with_ct(transport, cancel).await {
                    Ok(running) => {
                        let _ = running.waiting().await;
                    }
                    Err(error) => tracing::warn!(
                        target: "magistr::mcp",
                        "an attached MCP session did not start: {error}"
                    ),
                }
            })
        });
        OwnerEndpoint::serve_mcp(Arc::clone(&self.authority), acceptor)
            .await
            .map(Some)
    }

    /// No endpoint: local attachment needs Unix domain sockets, so a
    /// second `magistr mcp-serve` on this project is refused instead of
    /// proxied.
    ///
    /// # Errors
    /// Never; the signature matches the Unix endpoint.
    #[cfg(not(unix))]
    pub async fn serve_owner_endpoint(&self) -> io::Result<Option<OwnerEndpoint>> {
        tracing::debug!(
            target: "magistr::mcp",
            "no owner endpoint on this platform: a second server in this project is refused"
        );
        Ok(None)
    }
}
