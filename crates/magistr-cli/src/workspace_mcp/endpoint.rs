//! The workspace service over Magistr's existing local owner transport.
use std::{io, sync::Arc};

use magistr_engine::native::{McpAcceptor, OwnerEndpoint};
use magistr_workspace::service::WorkspaceService;
use rmcp::ServiceExt;

use super::WorkspaceMcpService;
use crate::mcp_tools::protocol_cap::ProtocolCap;

pub(crate) async fn serve(service: Arc<WorkspaceService>) -> io::Result<OwnerEndpoint> {
    let root = service.root().to_owned();
    let generation = service.generation().get();
    let acceptor: McpAcceptor = Arc::new(move |stream, cancel, _context| {
        let service = Arc::clone(&service);
        Box::pin(async move {
            // Construct, never clone, a connection handler: automatic identities
            // must differ even while the underlying writer is shared.
            let handler = match WorkspaceMcpService::new(service) {
                Ok(handler) => handler,
                Err(error) => {
                    tracing::warn!(%error, "workspace session admission failed");
                    return;
                }
            };
            let complete = handler.completion();
            let transport = ProtocolCap::new(
                &handler,
                rmcp::transport::IntoTransport::<rmcp::service::RoleServer, io::Error, _>::into_transport(
                    stream,
                ),
            );
            match handler.serve_with_ct(transport, cancel).await {
                Ok(running) => {
                    let _ = running.waiting().await;
                }
                Err(error) => tracing::debug!(%error, "attached workspace MCP session ended"),
            }
            // rmcp's loop has a bounded response drain; its spawned request
            // tasks can outlive that waiter. The handler guard joins them too.
            complete.cancelled().await;
        })
    });
    OwnerEndpoint::serve_workspace(&root, generation, acceptor).await
}
