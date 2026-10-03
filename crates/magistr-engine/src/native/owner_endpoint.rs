//! The local endpoint at which clients reach one admitted owner.

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[cfg(any(unix, windows))]
use super::owner_connections::OwnerConnections;
#[cfg(windows)]
use super::owner_pipe::PipeListener;
#[cfg(unix)]
use super::owner_socket_file::OwnerSocketFile;
use super::{
    McpAcceptor, NativeEngines, OwnerGeneration, OwnerIdentity, ProjectAuthority,
    workspace_socket_path,
};

/// A local endpoint backed by a workflow admission or an already admitted
/// workspace. Each MCP connection is a complete session of the same owner.
/// Workspace identity and generation are independent of workflow writer roles.
///
/// The endpoint is embedded in its host invocation. Dropping it stops
/// admission and signals every connection to close. Hosts must call
/// [`Self::shutdown`] before releasing owned services to join their cleanup.
/// Clients never acquire a writer merely because attachment failed.
pub struct OwnerEndpoint {
    path: PathBuf,
    generation: OwnerGeneration,
    stop: CancellationToken,
    accept: Option<JoinHandle<()>>,
    #[cfg(unix)]
    _socket: OwnerSocketFile,
    #[cfg(any(unix, windows))]
    _shared: Arc<OwnerConnections>,
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
    /// Serve typed commands on the admitted workflow writer.
    ///
    /// # Errors
    /// As [`Self::serve_with`].
    pub async fn serve(engines: NativeEngines) -> io::Result<Self> {
        Self::serve_with(engines, None).await
    }

    /// Serve typed commands and optionally the host's MCP session.
    ///
    /// # Errors
    /// A live or non-socket endpoint is preserved and refused. Only a stale
    /// socket is reclaimed. Local attachment requires Unix domain sockets.
    pub async fn serve_with(engines: NativeEngines, mcp: Option<McpAcceptor>) -> io::Result<Self> {
        Self::serve_workflow(engines.shared_authority(), Some(engines), mcp).await
    }

    /// Serve an MCP session over a bare workflow admission.
    ///
    /// # Errors
    /// As [`Self::serve_with`].
    pub async fn serve_mcp(authority: Arc<ProjectAuthority>, mcp: McpAcceptor) -> io::Result<Self> {
        Self::serve_workflow(authority, None, Some(mcp)).await
    }

    /// Serve the MCP adapter for an already admitted workspace owner.
    /// The caller retains its OS-held workspace service until this endpoint's
    /// [`Self::shutdown`] completes. This method takes no workflow lease and
    /// opens no workspace state. A generation of zero is never an admission.
    ///
    /// # Errors
    /// As [`Self::serve_with`], plus canonical-root and private-directory
    /// failures, or an invalid zero generation.
    pub async fn serve_workspace(
        root: &Path,
        generation: u64,
        mcp: McpAcceptor,
    ) -> io::Result<Self> {
        let root = root.canonicalize()?;
        let path = workspace_socket_path(&root)?;
        let generation = OwnerGeneration::admitted(generation)?;
        Self::serve_at(
            OwnerIdentity::Workspace { root },
            path,
            generation,
            None,
            None,
            Some(mcp),
        )
        .await
    }

    /// Stop accepting, cancel every connection and join each MCP service's
    /// cleanup before releasing the endpoint's references to its owner.
    pub async fn shutdown(mut self) -> io::Result<()> {
        self.stop.cancel();
        if let Some(accept) = self.accept.take() {
            accept.await.map_err(io::Error::other)?;
        }
        Ok(())
    }

    /// The socket clients connect to.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The generation returned by this endpoint's admission handshake.
    #[must_use]
    pub const fn generation(&self) -> OwnerGeneration {
        self.generation
    }

    async fn serve_workflow(
        authority: Arc<ProjectAuthority>,
        engines: Option<NativeEngines>,
        mcp: Option<McpAcceptor>,
    ) -> io::Result<Self> {
        let identity = OwnerIdentity::Workflow {
            project_id: authority.project_id().to_owned(),
            role: authority.role(),
        };
        let path = authority.owner_socket_path();
        let generation = authority.generation();
        Self::serve_at(identity, path, generation, Some(authority), engines, mcp).await
    }

    #[cfg(unix)]
    async fn serve_at(
        identity: OwnerIdentity,
        path: PathBuf,
        generation: OwnerGeneration,
        authority: Option<Arc<ProjectAuthority>>,
        engines: Option<NativeEngines>,
        mcp: Option<McpAcceptor>,
    ) -> io::Result<Self> {
        let private_directory = matches!(identity, OwnerIdentity::Workspace { .. });
        let (socket, listener) = OwnerSocketFile::bind(&path, private_directory).await?;
        let shared = Arc::new(OwnerConnections::new(
            identity,
            authority,
            engines,
            generation,
            socket.uid(),
            mcp,
        ));
        let stop = CancellationToken::new();
        let accept = tokio::spawn(OwnerConnections::serve(
            listener,
            Arc::downgrade(&shared),
            stop.clone(),
        ));
        Ok(Self {
            path,
            generation,
            stop,
            accept: Some(accept),
            _socket: socket,
            _shared: shared,
        })
    }

    /// The Windows endpoint: a pipe granted to this process's user alone.
    /// Only a workspace owner serves one; the workflow owner's clients
    /// attach synchronously and have no pipe transport yet, so it is
    /// refused rather than served to nobody.
    #[cfg(windows)]
    async fn serve_at(
        identity: OwnerIdentity,
        path: PathBuf,
        generation: OwnerGeneration,
        authority: Option<Arc<ProjectAuthority>>,
        engines: Option<NativeEngines>,
        mcp: Option<McpAcceptor>,
    ) -> io::Result<Self> {
        if !matches!(identity, OwnerIdentity::Workspace { .. }) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "magistr: the workflow owner endpoint needs Unix domain sockets",
            ));
        }
        let user = super::user_sid::current_user()?;
        let listener = PipeListener::bind(&path, &user)?;
        let shared = Arc::new(OwnerConnections::new(
            identity, authority, engines, generation, user, mcp,
        ));
        let stop = CancellationToken::new();
        let accept = tokio::spawn(OwnerConnections::serve(
            listener,
            Arc::downgrade(&shared),
            stop.clone(),
        ));
        Ok(Self {
            path,
            generation,
            stop,
            accept: Some(accept),
            _shared: shared,
        })
    }

    #[cfg(not(any(unix, windows)))]
    async fn serve_at(
        _identity: OwnerIdentity,
        _path: PathBuf,
        _generation: OwnerGeneration,
        _authority: Option<Arc<ProjectAuthority>>,
        _engines: Option<NativeEngines>,
        _mcp: Option<McpAcceptor>,
    ) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "magistr: the owner endpoint needs Unix domain sockets",
        ))
    }
}

impl Drop for OwnerEndpoint {
    fn drop(&mut self) {
        // Detach the accept-loop handle so it can finish cancellation and
        // joined cleanup. Aborting it would detach MCP service work again.
        self.stop.cancel();
    }
}

#[cfg(all(test, unix))]
#[path = "owner_endpoint.tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "owner_workspace_windows.tests.rs"]
mod windows_tests;

#[cfg(all(test, unix))]
#[path = "owner_workspace.tests.rs"]
mod workspace_tests;
