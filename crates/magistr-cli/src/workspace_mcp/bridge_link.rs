//! The bridge's side of one owner: an in-process service over a duplex, or
//! the socket a running owner welcomed this process on.

use std::sync::Arc;

use crate::mcp_bridge::Frames;
use color_eyre::eyre;
use magistr_workspace::service::{WorkspaceOpenOptions, WorkspaceService};
use rmcp::{ServiceExt, service::ServerInitializeError};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{WorkspaceMcpService, connection::WorkspaceConnection};
use crate::mcp_tools::protocol_cap::ProtocolCap;

/// One reachable owner. Lines go in and out exactly as MCP frames them; the
/// bridge never reorders or batches them.
pub(super) struct Link {
    frames: Frames,
    embedded: Option<Embedded>,
    generation: u64,
}

/// The owner this process is: the admitted service, the endpoint other
/// bridges attach to and the rmcp session serving this bridge's own client.
struct Embedded {
    service: Arc<WorkspaceService>,
    #[cfg(any(unix, windows))]
    endpoint: Option<magistr_engine::native::OwnerEndpoint>,
    complete: CancellationToken,
    serving: JoinHandle<eyre::Result<()>>,
}

impl Link {
    /// Own or attach, within the bounded startup window. The error is the
    /// admission's own refusal (legacy marker, held lock without a
    /// listener, refused handshake) and carries its exit code.
    pub(super) async fn open(
        options: &WorkspaceOpenOptions,
    ) -> Result<Self, magistr_workspace::service::WorkspaceOpenError> {
        let connection = WorkspaceConnection::open(options.clone()).await?;
        Self::from_connection(connection).await.map_err(|error| {
            magistr_workspace::service::WorkspaceOpenError::Locked(error.to_string())
        })
    }

    async fn from_connection(connection: WorkspaceConnection) -> eyre::Result<Self> {
        match connection {
            WorkspaceConnection::Owner(service) => Self::embed(service).await,
            #[cfg(any(unix, windows))]
            WorkspaceConnection::Attached { generation, stream } => Ok(Self {
                frames: Frames::new(stream),
                embedded: None,
                generation: generation.get(),
            }),
        }
    }

    async fn embed(service: Arc<WorkspaceService>) -> eyre::Result<Self> {
        #[cfg(any(unix, windows))]
        let endpoint = match super::endpoint::serve(Arc::clone(&service)).await {
            Ok(endpoint) => endpoint,
            Err(error) => {
                shutdown_service(service).await?;
                return Err(error.into());
            }
        };
        let handler = match WorkspaceMcpService::new(Arc::clone(&service)) {
            Ok(handler) => handler,
            Err(error) => {
                #[cfg(any(unix, windows))]
                endpoint.shutdown().await?;
                shutdown_service(service).await?;
                return Err(eyre::eyre!("{error}"));
            }
        };
        let complete = handler.completion();
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_side);
        let serving = tokio::spawn(async move {
            let transport = ProtocolCap::new(
                &handler,
                rmcp::transport::IntoTransport::<rmcp::service::RoleServer, std::io::Error, _>::into_transport((
                    server_read,
                    server_write,
                )),
            );
            match handler.serve(transport).await {
                Ok(running) => running
                    .waiting()
                    .await
                    .map(|_| ())
                    .map_err(eyre::Report::from),
                Err(ServerInitializeError::ConnectionClosed(_)) => Ok(()),
                Err(error) => Err(eyre::Report::from(error)),
            }
        });
        let generation = service.generation().get();
        Ok(Self {
            frames: Frames::new(client_side),
            embedded: Some(Embedded {
                service,
                #[cfg(any(unix, windows))]
                endpoint: Some(endpoint),
                complete,
                serving,
            }),
            generation,
        })
    }

    /// Whether this process is the owner behind the link.
    pub(super) fn is_embedded(&self) -> bool {
        self.embedded.is_some()
    }

    /// The owner generation this link reaches.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    /// The next owner line; `None` once the owner is gone.
    pub(super) async fn next_line(&mut self) -> Option<String> {
        self.frames.next_line().await
    }

    /// Send one client line to the owner.
    pub(super) async fn send(&mut self, line: &str) -> std::io::Result<()> {
        self.frames.send(line).await
    }

    /// The client is gone: stop sending, so the owner sees EOF and answers
    /// what is still in flight before it closes its side. Returns whether
    /// the owner will see that EOF: a Windows pipe has no half-close, so an
    /// attached owner there keeps its side open and the caller must stop
    /// reading once its last answer is in.
    pub(super) async fn half_close(&mut self) -> bool {
        self.frames.half_close().await;
        self.embedded.is_some() || cfg!(unix)
    }

    /// End the link: an attached socket is simply closed; an embedded owner
    /// releases its client session, stops its endpoint and shuts the
    /// workspace down in the order the service requires.
    pub(super) async fn close(self) -> eyre::Result<()> {
        let Self {
            frames, embedded, ..
        } = self;
        drop(frames);
        let Some(owner) = embedded else {
            return Ok(());
        };
        let served = owner.serving.await.map_err(eyre::Report::from)?;
        owner.complete.cancelled().await;
        #[cfg(any(unix, windows))]
        if let Some(endpoint) = owner.endpoint {
            endpoint.shutdown().await?;
        }
        shutdown_service(owner.service).await?;
        served
    }
}

async fn shutdown_service(service: Arc<WorkspaceService>) -> eyre::Result<()> {
    let service = Arc::try_unwrap(service).map_err(|_| {
        eyre::eyre!(
            "workspace service references remain after all MCP handlers stopped; explicit service shutdown refused"
        )
    })?;
    service.shutdown().await?;
    Ok(())
}

#[cfg(test)]
#[path = "bridge_link.tests.rs"]
mod tests;

impl crate::mcp_bridge::BridgeLink for Link {
    type Options = WorkspaceOpenOptions;
    async fn open(options: &Self::Options) -> Result<Self, String> {
        Self::open(options).await.map_err(|e| e.to_string())
    }
    fn generation(&self) -> u64 {
        self.generation()
    }
    fn is_embedded(&self) -> bool {
        self.is_embedded()
    }
    async fn next_line(&mut self) -> Option<String> {
        self.next_line().await
    }
    async fn send(&mut self, line: &str) -> std::io::Result<()> {
        self.send(line).await
    }
    async fn half_close(&mut self) -> bool {
        self.half_close().await
    }
    async fn close(self) -> eyre::Result<()> {
        self.close().await
    }
}
