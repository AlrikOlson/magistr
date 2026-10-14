//! Workflow and MCP clients bind to their admitted working-copy execution.
use super::service::MagistrService;
use magistr_engine::native::OwnerEndpoint;
use std::io;
#[cfg(any(unix, windows))]
use {
    super::protocol_cap::ProtocolCap, magistr_engine::native::McpAcceptor, rmcp::ServiceExt as _,
    std::sync::Arc,
};

impl MagistrService {
    /// Serve MCP and typed commands through this single project writer.
    /// The endpoint binds each connection before welcoming it.
    #[cfg(any(unix, windows))]
    pub async fn serve_owner_endpoint(&self) -> io::Result<Option<OwnerEndpoint>> {
        #[cfg(unix)]
        self.engines.publish().await?;
        let acceptor: McpAcceptor = Arc::new(move |stream, cancel, engines| {
            Box::pin(async move {
                let Some(engines) = engines else {
                    return;
                };
                let service = MagistrService::from_engines(engines);
                let transport = ProtocolCap::new(&service,
                    rmcp::transport::IntoTransport::<rmcp::service::RoleServer, io::Error, _>::into_transport(stream));
                match service.serve_with_ct(transport, cancel).await {
                    Ok(running) => {
                        let _ = running.waiting().await;
                    }
                    Err(error) => tracing::warn!("An attached session did not start: {error}"),
                }
            })
        });
        OwnerEndpoint::serve_with(self.engines.clone(), Some(acceptor))
            .await
            .map(Some)
    }

    #[cfg(not(any(unix, windows)))]
    pub async fn serve_owner_endpoint(&self) -> io::Result<Option<OwnerEndpoint>> {
        Ok(None)
    }
}
