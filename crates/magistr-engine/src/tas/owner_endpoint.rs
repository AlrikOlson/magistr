//! The owner's endpoint: where attached surfaces reach the one writer.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::task::JoinHandle;

use super::mcp_acceptor::McpAcceptor;
use super::native_engines::NativeEngines;
use super::owner_generation::OwnerGeneration;
use super::project_authority::ProjectAuthority;

/// A Unix socket an admitted writer serves so that a second surface on the
/// same project — another cockpit, another `magistr mcp-serve` — acts
/// through this writer instead of opening a cached one of its own.
///
/// The endpoint lives exactly as long as this value: the owner is an
/// embedded host, alive for its CLI or session invocation, not a daemon.
/// Dropping the endpoint stops accepting, hangs up every attached client,
/// and unlinks the socket; the attached surfaces see EOF and stand down or
/// reconnect to whoever owns the project next. Nothing an attached client
/// does keeps the owner alive, and the owner never waits for a client.
///
/// Every connection is admitted the same way: the peer must be the owner's
/// user (the socket's credential, not the client's word), the `Hello` must
/// name this owner's protocol version, project and role, and the kind of
/// attachment must be one this owner offers. Typed commands then run on the
/// owner's [`NativeEngines`] through [`NativeEngines::apply`], fenced by the
/// [`OwnerGeneration`] the admission minted; an MCP attachment is handed to
/// the host's acceptor. A host serves what it has: a writer
/// ([`Self::serve`], [`Self::serve_with`]) or an MCP session over a bare
/// admission ([`Self::serve_mcp`]), and refuses the other kind.
pub struct OwnerEndpoint {
    path: PathBuf,
    generation: OwnerGeneration,
    accept: JoinHandle<()>,
    /// The writer and the wiring every connection task reaches through a
    /// `Weak`: dropping the endpoint releases the writer at once, however
    /// many clients are still attached.
    _shared: Arc<unix::Shared>,
}

impl std::fmt::Debug for OwnerEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnerEndpoint")
            .field("path", &self.path)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl OwnerEndpoint {
    /// Serve typed commands on `engines`' endpoint.
    ///
    /// # Errors
    /// As [`Self::serve_with`].
    pub async fn serve(engines: NativeEngines) -> io::Result<Self> {
        Self::serve_with(engines, None).await
    }

    /// Serve typed commands on `engines`' endpoint, and the host's MCP
    /// session through `mcp` when it offers one.
    ///
    /// # Errors
    /// `AddrInUse` when another endpoint is live at the path (an owner of
    /// this generation cannot exist twice, so this is a leftover runtime
    /// still winding down: wait for it); `Unsupported` off Unix; any other
    /// failure creating or binding the socket. A stale socket file with no
    /// listener behind it is reclaimed.
    pub async fn serve_with(engines: NativeEngines, mcp: Option<McpAcceptor>) -> io::Result<Self> {
        unix::serve(engines.authority_arc(), Some(engines), mcp).await
    }

    /// Serve only the host's MCP session on `authority`'s endpoint — what
    /// `magistr mcp-serve` offers, whose writer is its rmcp service rather
    /// than a [`NativeEngines`]. Typed commands are refused at the
    /// handshake.
    ///
    /// # Errors
    /// As [`Self::serve_with`].
    pub async fn serve_mcp(authority: Arc<ProjectAuthority>, mcp: McpAcceptor) -> io::Result<Self> {
        unix::serve(authority, None, Some(mcp)).await
    }

    /// The socket clients connect to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The generation every command must carry.
    #[must_use]
    pub const fn generation(&self) -> OwnerGeneration {
        self.generation
    }
}

impl Drop for OwnerEndpoint {
    fn drop(&mut self) {
        // Aborting the accept loop drops its JoinSet, which aborts every
        // connection task; the socket file is ours to remove.
        self.accept.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
mod unix {
    use std::io;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::{Arc, Weak};
    use std::time::Duration;

    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::task::JoinSet;

    use super::OwnerEndpoint;
    use crate::tas::attach_kind::AttachKind;
    use crate::tas::attach_refusal::AttachRefusal;
    use crate::tas::mcp_acceptor::McpAcceptor;
    use crate::tas::native_engines::NativeEngines;
    use crate::tas::owner_generation::OwnerGeneration;
    use crate::tas::owner_wire::{
        ClientFrame, CommandRefusal, Greeting, Hello, OWNER_PROTOCOL_VERSION, ReplyFrame,
    };
    use crate::tas::project_authority::ProjectAuthority;

    /// The longest handshake line accepted before the connection is
    /// dropped as not the protocol.
    const MAX_HELLO: usize = 64 * 1024;
    /// How long a probe of an existing socket file waits for an answer.
    const PROBE: Duration = Duration::from_millis(250);

    pub(super) struct Shared {
        authority: Arc<ProjectAuthority>,
        /// The typed-command writer, when this owner has one.
        engines: Option<NativeEngines>,
        generation: OwnerGeneration,
        uid: u32,
        mcp: Option<McpAcceptor>,
    }

    pub(super) async fn serve(
        authority: Arc<ProjectAuthority>,
        engines: Option<NativeEngines>,
        mcp: Option<McpAcceptor>,
    ) -> io::Result<OwnerEndpoint> {
        let path = authority.owner_socket_path();
        let generation = authority.generation();
        let failure = |error: io::Error| {
            io::Error::new(
                error.kind(),
                format!(
                    "magistr: cannot serve the owner endpoint at {}: {error}",
                    path.display()
                ),
            )
        };
        ensure_endpoint_free(&path).await.map_err(failure)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(failure)?;
        }
        let listener = UnixListener::bind(&path).map_err(failure)?;
        // Only the owner's user may connect: the file mode gates
        // `connect(2)`, and the peer credential is checked again per
        // connection.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(failure)?;
        let uid = std::fs::metadata(&path).map_err(failure)?.uid();
        let shared = Arc::new(Shared {
            authority,
            engines,
            generation,
            uid,
            mcp,
        });
        let accept = tokio::spawn(accept_loop(listener, Arc::downgrade(&shared)));
        Ok(OwnerEndpoint {
            path,
            generation,
            accept,
            _shared: shared,
        })
    }

    /// Refuse a live listener; reclaim a socket file nothing answers on.
    async fn ensure_endpoint_free(path: &std::path::Path) -> io::Result<()> {
        match tokio::time::timeout(PROBE, UnixStream::connect(path)).await {
            Ok(Err(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                match std::fs::remove_file(path) {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(error),
                }
            }
            result => {
                let detail = match result {
                    Ok(Ok(_)) => "another owner endpoint is listening".to_owned(),
                    Ok(Err(error)) => error.to_string(),
                    Err(_) => "availability probe timed out".to_owned(),
                };
                Err(io::Error::new(io::ErrorKind::AddrInUse, detail))
            }
        }
    }

    async fn accept_loop(listener: UnixListener, shared: Weak<Shared>) {
        // Dropped with this task when the endpoint is dropped, aborting
        // every connection it spawned.
        let mut connections = JoinSet::new();
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    connections.spawn(serve_connection(stream, shared.clone()));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
            while connections.try_join_next().is_some() {}
        }
    }

    async fn serve_connection(mut stream: UnixStream, shared: Weak<Shared>) {
        let Some(owner) = shared.upgrade() else {
            return;
        };
        let Ok(hello) = read_hello(&mut stream).await else {
            return;
        };
        let greeting = match owner.admit(&stream, &hello) {
            Ok(()) => Greeting::Welcome {
                generation: owner.generation,
                owner_pid: std::process::id(),
            },
            Err(refusal) => Greeting::Refused(refusal),
        };
        let welcomed = matches!(greeting, Greeting::Welcome { .. });
        if write_line(&mut stream, &greeting).await.is_err() || !welcomed {
            return;
        }
        match hello.attach {
            AttachKind::Mcp => {
                if let Some(acceptor) = owner.mcp.clone() {
                    drop(owner);
                    acceptor(stream);
                }
            }
            AttachKind::Commands => {
                drop(owner);
                let _ = command_loop(stream, shared).await;
            }
        }
    }

    impl Shared {
        fn admit(&self, stream: &UnixStream, hello: &Hello) -> Result<(), AttachRefusal> {
            let peer = stream
                .peer_cred()
                .map_err(|error| AttachRefusal::Caller(error.to_string()))?;
            if peer.uid() != self.uid {
                return Err(AttachRefusal::Caller(format!(
                    "uid {} is not the owner's user",
                    peer.uid()
                )));
            }
            if hello.protocol != OWNER_PROTOCOL_VERSION {
                return Err(AttachRefusal::Protocol {
                    served: OWNER_PROTOCOL_VERSION,
                    offered: hello.protocol,
                });
            }
            if hello.project_id != self.authority.project_id() {
                return Err(AttachRefusal::Project {
                    expected: self.authority.project_id().to_owned(),
                    offered: hello.project_id.clone(),
                });
            }
            if hello.role != self.authority.role() {
                return Err(AttachRefusal::Role {
                    expected: self.authority.role(),
                    offered: hello.role,
                });
            }
            let offered = match hello.attach {
                AttachKind::Mcp => self.mcp.is_some(),
                AttachKind::Commands => self.engines.is_some(),
            };
            if !offered {
                return Err(AttachRefusal::Unsupported(hello.attach));
            }
            Ok(())
        }
    }

    /// One line, read without buffering past it: what follows a welcomed
    /// MCP handshake belongs to the rmcp session, not to this reader.
    async fn read_hello(stream: &mut UnixStream) -> io::Result<Hello> {
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            stream.read_exact(&mut byte).await?;
            if byte[0] == b'\n' {
                break;
            }
            line.push(byte[0]);
            if line.len() > MAX_HELLO {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "handshake line too long",
                ));
            }
        }
        serde_json::from_slice(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_line<T: serde::Serialize>(
        writer: &mut (impl AsyncWriteExt + Unpin),
        value: &T,
    ) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(value)?;
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;
        writer.flush().await
    }

    async fn command_loop(stream: UnixStream, shared: Weak<Shared>) -> io::Result<()> {
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        while let Some(line) = lines.next_line().await? {
            let frame: ClientFrame = serde_json::from_str(&line)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let ClientFrame::Command {
                id,
                generation,
                command,
            } = frame
            else {
                return Ok(());
            };
            // The endpoint is gone: hang up rather than answer for it.
            let Some(owner) = shared.upgrade() else {
                return Ok(());
            };
            let outcome = if generation != owner.generation {
                Err(CommandRefusal::StaleGeneration {
                    current: owner.generation,
                })
            } else if let Some(engines) = &owner.engines {
                engines.apply(*command).map_err(CommandRefusal::Engine)
            } else {
                Err(CommandRefusal::Engine(
                    "this owner serves no typed commands".to_owned(),
                ))
            };
            drop(owner);
            write_line(&mut write, &ReplyFrame { id, outcome }).await?;
        }
        Ok(())
    }
}

#[cfg(not(unix))]
mod unix {
    use std::io;

    use super::OwnerEndpoint;
    use std::sync::Arc;

    use crate::tas::mcp_acceptor::McpAcceptor;
    use crate::tas::native_engines::NativeEngines;
    use crate::tas::project_authority::ProjectAuthority;

    pub(super) struct Shared;

    pub(super) async fn serve(
        _authority: Arc<ProjectAuthority>,
        _engines: Option<NativeEngines>,
        _mcp: Option<McpAcceptor>,
    ) -> io::Result<OwnerEndpoint> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "magistr: the owner endpoint needs Unix domain sockets",
        ))
    }
}

#[cfg(all(test, unix))]
#[path = "owner_endpoint.tests.rs"]
mod tests;
