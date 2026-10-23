//! Host-owned workflow readers, admitted writers and ordered session observations.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use magistr_engine::{
    CallerIdentity, NativeConfig, NativeEngines, NativeReader, OwnerClient, OwnerEndpoint,
    OwnerGeneration, ProjectAuthority, WriterRole,
};
use tokio::task::JoinHandle;

use super::attached_port::AttachedEnginePort;
use super::error::EngineError;
use super::feed::spawn_feed;
use super::feed_link::FeedLink;
use super::native_port::NativeEnginePort;
use super::read_snapshot::SnapshotReads;
use super::reseed::spawn_reseed;
use magistr_cockpit::engine::EnginePort;
use magistr_cockpit_runtime::channel::Inbox;

/// The project's magistr engines, in-process, and the socket their
/// live feed attaches to.
///
/// Opening validates host admission before an interface starts using the client.
/// [`EngineClient::with_config`]
/// admits this process as the project's host and holds the writer — one
/// project writer across cockpit, loop and MCP hosts — while [`EngineClient::observe`]
/// holds a reader only, so a viewer never contends with a running host.
/// [`EngineClient::admit_or_attach_config`] is the cockpit's own entry: the host
/// when the role is free, otherwise a client on the running host's owner
/// endpoint ([`OwnerClient`]) acting through that host's writer — and a
/// refusal when neither is possible, never a second writer.
#[derive(Clone, Debug)]
pub struct EngineClient {
    reader: NativeReader,
    reads: SnapshotReads,
    /// The admitted writer, when this client was opened or attached as a
    /// host rather than as an observer.
    writer: Option<NativeEngines>,
    /// The running host's owner endpoint, when this client attached to it
    /// instead of holding a writer. Shared with the engine port, which
    /// speaks on it one verb at a time.
    attached: Option<Arc<Mutex<OwnerClient>>>,
    socket: PathBuf,
    /// Coalesced requests to read the owning stores after changes or missed frames.
    link: FeedLink,
}

impl EngineClient {
    /// Admit this process as the host of the project the configuration
    /// names and open its engines. The configuration must carry the
    /// project's verification evidence capability — the host attaches it
    /// through its writer config — because a host records gates and
    /// completes chunks, and neither may happen unbound from the source;
    /// a configuration without it is refused before any lease is taken.
    ///
    /// # Errors
    ///
    /// The root is not a directory, the store's directory cannot be
    /// created, the configuration carries no evidence capability, or the
    /// host lease is refused because another cockpit or loop holds this
    /// project.
    pub fn with_config(config: NativeConfig) -> Result<Self, EngineError> {
        Self::store_ready(&config)?;
        if config.evidence().is_none() {
            return Err(EngineError::Admission {
                root: config.project_root().to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the writer configuration carries no verification evidence capability; \
                     a host must be admitted on a writer config",
                ),
            });
        }
        let engines = ProjectAuthority::admit(&config, WriterRole::Host)
            .map_err(|source| EngineError::Admission {
                root: config.project_root().to_path_buf(),
                source,
            })?
            .open();
        Ok(Self::attach(engines))
    }

    /// The host of the project the configuration names when the role is
    /// free; otherwise a client attached to the running host's owner
    /// endpoint, acting through that host's writer. Fails closed when the
    /// role is held and no endpoint answers: the caller may run the
    /// observer cockpit ([`Self::observe`]) explicitly, but nothing here
    /// opens a second writer.
    ///
    /// # Errors
    ///
    /// As [`Self::with_config`], except that a held host lease becomes
    /// [`EngineError::Occupied`] only when attaching failed too — the
    /// refusal carries both reasons.
    pub fn admit_or_attach_config(config: NativeConfig) -> Result<Self, EngineError> {
        let (root, lease) = match Self::with_config(config.clone()) {
            Ok(host) => return Ok(host),
            Err(EngineError::Admission { root, source })
                if source.kind() == io::ErrorKind::WouldBlock =>
            {
                (root, source)
            }
            Err(other) => return Err(other),
        };
        match OwnerClient::attach_project(
            &config,
            WriterRole::Host,
            CallerIdentity::this_process("cockpit"),
        ) {
            Ok(client) => {
                let reader = NativeReader::open(&config);
                Ok(Self {
                    socket: config.feed_socket_path(),
                    reads: SnapshotReads::new(reader.clone()),
                    reader,
                    writer: None,
                    attached: Some(Arc::new(Mutex::new(client))),
                    link: FeedLink::new(),
                })
            }
            Err(attach) => Err(EngineError::Occupied {
                root,
                lease,
                attach,
            }),
        }
    }

    /// Observe the project at `root` without admission: a reader for the
    /// seed, the feed and the re-seed timer, and no writer. The launcher and
    /// the bare roadmap cockpit open this way, so they run beside a loop or
    /// a `run` that holds the host lease.
    ///
    /// # Errors
    ///
    /// `root` is not a directory, or the store's directory cannot be
    /// created.
    pub fn observe(root: impl Into<PathBuf>) -> Result<Self, EngineError> {
        Self::observe_config(NativeConfig::from_env(root))
    }

    /// [`Self::observe`] on an explicit store configuration.
    ///
    /// # Errors
    ///
    /// As [`Self::observe`].
    pub fn observe_config(config: NativeConfig) -> Result<Self, EngineError> {
        Self::store_ready(&config)?;
        let reader = NativeReader::open(&config);
        Ok(Self {
            socket: config.feed_socket_path(),
            reads: SnapshotReads::new(reader.clone()),
            reader,
            writer: None,
            attached: None,
            link: FeedLink::new(),
        })
    }

    /// The client over a writer the entry point already admitted — the loop
    /// driver's, so a cockpit run beside a loop never opens a second cached
    /// writer on the store.
    #[must_use]
    pub fn attach(engines: NativeEngines) -> Self {
        let reader = engines.reader();
        Self {
            socket: engines.config().feed_socket_path(),
            reads: SnapshotReads::new(reader.clone()),
            reader,
            writer: Some(engines),
            attached: None,
            link: FeedLink::new(),
        }
    }

    fn store_ready(config: &NativeConfig) -> Result<(), EngineError> {
        let root = config.project_root();
        if !root.is_dir() {
            return Err(EngineError::NoProject(root.to_path_buf()));
        }
        if config.persistence_enabled() {
            std::fs::create_dir_all(config.data_dir()).map_err(|source| EngineError::Store {
                path: config.data_dir().to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }

    /// The read capability every client holds.
    #[must_use]
    pub const fn reader(&self) -> &NativeReader {
        &self.reader
    }

    /// The admitted writer, `None` for an observer or an attached client.
    #[must_use]
    pub const fn writer(&self) -> Option<&NativeEngines> {
        self.writer.as_ref()
    }

    /// The generation of the running host this client is attached to,
    /// `None` when it holds a writer of its own or observes.
    #[must_use]
    pub fn attached_generation(&self) -> Option<OwnerGeneration> {
        self.attached.as_ref().map(|client| {
            client
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .generation()
        })
    }

    /// The engine port the session acts through: the in-process writer
    /// when this client is the host, the running host's endpoint when it
    /// attached, and nothing for an observer.
    #[must_use]
    pub fn port(&self) -> Option<Box<dyn EnginePort>> {
        if let Some(writer) = &self.writer {
            return Some(Box::new(NativeEnginePort::new(writer.clone())));
        }
        self.attached.as_ref().map(|client| {
            Box::new(AttachedEnginePort::new(Arc::clone(client))) as Box<dyn EnginePort>
        })
    }

    /// Serve this host's owner endpoint, so a second cockpit on the project
    /// attaches to this writer instead of being refused. `None` when this
    /// client holds no writer, or on a platform with neither Unix sockets
    /// nor named pipes, where the session runs without one. The endpoint lives as long as the handle:
    /// keep it beside the runtime and let it drop before the client does.
    ///
    /// # Errors
    ///
    /// The endpoint cannot be served — another listener is live at the
    /// socket, or it cannot be bound.
    pub async fn serve_owner(&self) -> Result<Option<OwnerEndpoint>, EngineError> {
        let Some(writer) = &self.writer else {
            return Ok(None);
        };
        crate::mcp_tools::MagistrService::from_engines(writer.clone())
            .serve_owner_endpoint()
            .await
            .map_err(|source| EngineError::Endpoint {
                path: writer.authority().owner_socket_path(),
                source,
            })
    }

    /// The broadcast socket the feed attaches to.
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Attach the live feed: every event on the socket and every change
    /// of attachment reaches `inbox` as `Msg::Engine`, reconnecting for as
    /// long as the loop lives. Abort the handle to stop it early.
    pub fn subscribe(&self, inbox: Inbox) -> JoinHandle<()> {
        spawn_feed(
            self.socket.clone(),
            self.reader.config().execution_context().key(),
            inbox,
            self.link.clone(),
        )
    }

    pub(crate) fn snapshot_reads(&self) -> SnapshotReads {
        self.reads.clone()
    }

    /// Coalesced read requests shared with the session observation.
    pub(crate) fn snapshot_requests(&self) -> FeedLink {
        self.link.clone()
    }

    /// Validate all owning stores periodically, including while a feed is attached.
    pub const RESEED: Duration = Duration::from_secs(15);

    /// Publish changed observations and requested reads unless newer changes supersede them.
    /// The baseline includes each family's successful data or read failure.
    pub fn reseed(&self, inbox: Inbox) -> JoinHandle<()> {
        spawn_reseed(self.reads.clone(), self.link.clone(), inbox, Self::RESEED)
    }
}

#[cfg(test)]
#[path = "client.tests.rs"]
mod tests;
