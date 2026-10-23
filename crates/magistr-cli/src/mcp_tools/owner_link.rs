//! Workflow owner admission and orderly settlement behind the shared MCP bridge.
use color_eyre::eyre;
#[cfg(any(unix, windows))]
use magistr_engine::native::{
    AttachError, CallerIdentity, LocalStream, OwnerClient, OwnerGeneration, WriterRole,
};
use magistr_engine::native::{NativeConfig, OwnerEndpoint};
use rmcp::{ServiceExt, service::ServerInitializeError};
use std::path::PathBuf;
#[cfg(any(unix, windows))]
use std::time::Duration;
use tokio::task::JoinHandle;

use super::{MagistrService, protocol_cap::ProtocolCap};
use crate::mcp_bridge::{BridgeLink, Frames};

pub(super) struct Options {
    pub root: PathBuf,
    pub config: NativeConfig,
}

pub(super) struct Link {
    frames: Frames,
    embedded: Option<Embedded>,
    generation: u64,
}

struct Embedded {
    endpoint: Option<OwnerEndpoint>,
    serving: JoinHandle<eyre::Result<()>>,
}

impl Link {
    async fn embed(service: MagistrService) -> eyre::Result<Self> {
        let endpoint = service.serve_owner_endpoint().await?;
        let generation = service.authority.generation().get();
        let (client, server) = tokio::io::duplex(64 * 1024);
        let serving = tokio::spawn(async move {
            let transport = ProtocolCap::new(
                &service,
                rmcp::transport::IntoTransport::<rmcp::service::RoleServer, std::io::Error, _>::into_transport(server),
            );
            match service.serve(transport).await {
                Ok(running) => running
                    .waiting()
                    .await
                    .map(|_| ())
                    .map_err(eyre::Report::from),
                Err(ServerInitializeError::ConnectionClosed(_)) => Ok(()),
                Err(error) => Err(eyre::Report::from(error)),
            }
        });
        Ok(Self {
            frames: Frames::new(client),
            embedded: Some(Embedded { endpoint, serving }),
            generation,
        })
    }
}

impl BridgeLink for Link {
    type Options = Options;

    async fn open(options: &Options) -> Result<Self, String> {
        #[cfg(any(unix, windows))]
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        #[cfg(not(any(unix, windows)))]
        return Self::embed(
            MagistrService::new(options.root.clone(), &options.config)
                .await
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(|e| e.to_string());
        #[cfg(any(unix, windows))]
        loop {
            match MagistrService::new(options.root.clone(), &options.config).await {
                Ok(service) => return Self::embed(service).await.map_err(|e| e.to_string()),
                Err(refusal) if refusal.kind() == std::io::ErrorKind::WouldBlock => {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if remaining.is_zero() {
                        return Err(format!(
                            "{refusal}; project attachment timed out after 5 seconds"
                        ));
                    }
                    match attach(&options.config, remaining.min(Duration::from_millis(250))).await?
                    {
                        Ok((generation, stream)) => {
                            return Ok(Self {
                                frames: Frames::new(stream),
                                embedded: None,
                                generation: generation.get(),
                            });
                        }
                        Err(AttachError::Unavailable(_) | AttachError::Deadline(_)) => {
                            tokio::time::sleep(
                                deadline
                                    .saturating_duration_since(tokio::time::Instant::now())
                                    .min(Duration::from_millis(50)),
                            )
                            .await;
                        }
                        Err(error) => {
                            return Err(format!("{refusal}; project attachment refused: {error}"));
                        }
                    }
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    }

    fn generation(&self) -> u64 {
        self.generation
    }
    fn is_embedded(&self) -> bool {
        self.embedded.is_some()
    }
    async fn next_line(&mut self) -> Option<String> {
        self.frames.next_line().await
    }
    async fn send(&mut self, line: &str) -> std::io::Result<()> {
        self.frames.send(line).await
    }
    async fn half_close(&mut self) -> bool {
        self.frames.half_close().await;
        self.embedded.is_some() || cfg!(unix)
    }
    async fn close(self) -> eyre::Result<()> {
        let Self {
            frames, embedded, ..
        } = self;
        drop(frames);
        if let Some(owner) = embedded {
            let served = owner.serving.await?;
            if let Some(endpoint) = owner.endpoint {
                endpoint.shutdown().await?;
            }
            served?;
        }
        Ok(())
    }
}

/// One bounded attempt to attach to the MCP session of the project's
/// service owner. The outer error is this process failing, the inner one
/// the owner's answer.
#[cfg(unix)]
async fn attach(
    config: &NativeConfig,
    deadline: Duration,
) -> Result<Result<(OwnerGeneration, LocalStream), AttachError>, String> {
    let config = config.clone();
    let attached = tokio::task::spawn_blocking(move || {
        OwnerClient::attach_mcp(
            &config,
            config.resolve_project_id(),
            WriterRole::Service,
            CallerIdentity::this_process("mcp"),
            deadline,
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    let (generation, stream) = match attached {
        Ok(attached) => attached,
        Err(error) => return Ok(Err(error)),
    };
    stream.set_nonblocking(true).map_err(|e| e.to_string())?;
    let stream = tokio::net::UnixStream::from_std(stream).map_err(|e| e.to_string())?;
    Ok(Ok((generation, stream)))
}

/// One bounded attempt to attach to the MCP session of the project's
/// service owner. The outer error is this process failing, the inner one
/// the owner's answer.
#[cfg(windows)]
async fn attach(
    config: &NativeConfig,
    deadline: Duration,
) -> Result<Result<(OwnerGeneration, LocalStream), AttachError>, String> {
    Ok(OwnerClient::attach_mcp(
        config,
        config.resolve_project_id(),
        WriterRole::Service,
        CallerIdentity::this_process("mcp"),
        deadline,
    )
    .await)
}
