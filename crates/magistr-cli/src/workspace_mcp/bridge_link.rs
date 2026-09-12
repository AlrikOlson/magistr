//! The bridge's side of one owner: an in-process service over a duplex, or
//! the socket a running owner welcomed this process on.

use std::{pin::Pin, sync::Arc, time::Duration};

use color_eyre::eyre;
use magistr_workspace::service::{WorkspaceOpenOptions, WorkspaceService};
use rmcp::{ServiceExt, service::ServerInitializeError};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{WorkspaceMcpService, connection::WorkspaceConnection};
use crate::mcp_tools::protocol_cap::ProtocolCap;

/// How long a replayed `initialize` may take a new owner.
const REPLAY_DEADLINE: Duration = Duration::from_secs(5);

type Reader = Lines<BufReader<Pin<Box<dyn AsyncRead + Send>>>>;
type Writer = Pin<Box<dyn AsyncWrite + Send>>;

/// One reachable owner. Lines go in and out exactly as MCP frames them; the
/// bridge never reorders or batches them.
pub(super) struct Link {
    reader: Reader,
    /// `None` once the client's side has been half-closed: the owner sees
    /// EOF, finishes what is in flight and closes its side in turn.
    writer: Option<Writer>,
    embedded: Option<Embedded>,
    generation: u64,
}

/// The owner this process is: the admitted service, the endpoint other
/// bridges attach to and the rmcp session serving this bridge's own client.
struct Embedded {
    service: Arc<WorkspaceService>,
    #[cfg(unix)]
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
            #[cfg(unix)]
            WorkspaceConnection::Attached { generation, stream } => {
                let (read, write) = stream.into_split();
                Ok(Self {
                    reader: BufReader::new(Box::pin(read) as Pin<Box<dyn AsyncRead + Send>>)
                        .lines(),
                    writer: Some(Box::pin(write)),
                    embedded: None,
                    generation: generation.get(),
                })
            }
        }
    }

    async fn embed(service: Arc<WorkspaceService>) -> eyre::Result<Self> {
        #[cfg(unix)]
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
                #[cfg(unix)]
                endpoint.shutdown().await?;
                shutdown_service(service).await?;
                return Err(eyre::eyre!("{error}"));
            }
        };
        let complete = handler.completion();
        let (client_side, server_side) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_side);
        let serving = tokio::spawn(async move {
            let transport = ProtocolCap::new(rmcp::transport::IntoTransport::<
                rmcp::service::RoleServer,
                std::io::Error,
                _,
            >::into_transport((
                server_read,
                server_write,
            )));
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
        let (client_read, client_write) = tokio::io::split(client_side);
        let generation = service.generation().get();
        Ok(Self {
            reader: BufReader::new(Box::pin(client_read) as Pin<Box<dyn AsyncRead + Send>>).lines(),
            writer: Some(Box::pin(client_write)),
            embedded: Some(Embedded {
                service,
                #[cfg(unix)]
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
        self.reader.next_line().await.ok().flatten()
    }

    /// Send one client line to the owner.
    pub(super) async fn send(&mut self, line: &str) -> std::io::Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the client's side of the link is closed",
            ));
        };
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await
    }

    /// The client is gone: stop sending, so the owner sees EOF and answers
    /// what is still in flight before it closes its side.
    pub(super) fn half_close(&mut self) {
        self.writer = None;
    }

    /// Initialize this owner on the client's behalf: send the recorded
    /// `initialize`, wait for its answer (owner lines that are not that answer
    /// are handed back for the client), then send `initialized`.
    pub(super) async fn replay(
        &mut self,
        id: &Value,
        initialize: &Value,
        initialized: &Value,
    ) -> eyre::Result<Vec<String>> {
        self.send(&initialize.to_string()).await?;
        let mut passthrough = Vec::new();
        let answered = tokio::time::timeout(REPLAY_DEADLINE, async {
            while let Some(line) = self.next_line().await {
                let message: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
                if message.get("id") == Some(id) && message.get("method").is_none() {
                    return message.get("error").is_none();
                }
                passthrough.push(line);
            }
            false
        })
        .await;
        match answered {
            Ok(true) => {}
            Ok(false) => eyre::bail!("the owner refused the replayed initialize"),
            Err(_) => eyre::bail!("the owner did not answer the replayed initialize in time"),
        }
        self.send(&initialized.to_string()).await?;
        Ok(passthrough)
    }

    /// End the link: an attached socket is simply closed; an embedded owner
    /// releases its client session, stops its endpoint and shuts the
    /// workspace down in the order the service requires.
    pub(super) async fn close(self) -> eyre::Result<()> {
        let Self {
            reader,
            writer,
            embedded,
            ..
        } = self;
        drop(reader);
        drop(writer);
        let Some(owner) = embedded else {
            return Ok(());
        };
        let served = owner.serving.await.map_err(eyre::Report::from)?;
        owner.complete.cancelled().await;
        #[cfg(unix)]
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
