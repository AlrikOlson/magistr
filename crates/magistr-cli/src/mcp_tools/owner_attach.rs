//! A second `magistr mcp-serve` on a project whose service owner is
//! already running: attach to that owner and speak its MCP session.
//!
//! Two agent sessions in one project used to end here: the second server
//! was refused the service lease and died, and its agent had no magistr
//! tools. Now the owner serves its rmcp session on the project's owner
//! endpoint, and the second server is a proxy — its stdio copied to and
//! from the socket the owner welcomed it on — so both agents act through
//! the one admitted writer. The proxy holds no lease and opens no engine;
//! when the owner goes away it ends with an error rather than becoming an
//! owner of its own.
//!
//! Attachment rides on Unix domain sockets. Elsewhere the owner serves no
//! endpoint and a second server is refused with the lease's own reason.

use std::io;
use std::path::Path;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;

use color_eyre::eyre;
use magistr_engine::native::{AttachError, OwnerEndpoint};
#[cfg(unix)]
use magistr_engine::native::{CallerIdentity, McpAcceptor, NativeConfig, OwnerClient, WriterRole};
#[cfg(unix)]
use rmcp::ServiceExt as _;

#[cfg(unix)]
use super::owner_stdio::ProxyStdio;
#[cfg(unix)]
use super::protocol_cap::ProtocolCap;
use super::service::MagistrService;

/// How long the handshake with the owner may take.
#[cfg(unix)]
const ATTACH_DEADLINE: Duration = Duration::from_secs(5);

/// The connection a welcomed attachment speaks MCP on.
#[cfg(unix)]
type Attached = std::os::unix::net::UnixStream;
/// No platform transport: an attachment is never welcomed here.
#[cfg(not(unix))]
type Attached = std::convert::Infallible;

impl MagistrService {
    /// Serve this owner's endpoint: an attached `magistr mcp-serve` gets a
    /// session of this same service over the socket, under the same
    /// protocol cap the stdio session runs under. The endpoint lives as
    /// long as the handle — keep it beside the stdio session and drop it
    /// when that session ends, so the attached sessions end with the
    /// owner.
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

/// Attach to the running service owner of `project_root` and proxy this
/// process's stdio to the owner's MCP session until one side ends.
///
/// Returns `Ok` when the client closed stdin — the session is over. Ends
/// with an error when the owner hung up first: the client is told the
/// owner is gone and can start a fresh server, which will then take the
/// lease itself.
///
/// # Errors
/// The owner endpoint did not take this process (it carries `refusal`,
/// the lease's own reason, beside the attachment's), or the proxy failed.
/// Without Unix domain sockets there is nothing to attach to, and the
/// refusal says so.
pub async fn proxy_to_owner(project_root: &Path, refusal: &io::Error) -> eyre::Result<()> {
    let attached = attach(project_root)
        .map_err(|attach| eyre::eyre!("{}", attach_refusal(refusal, &attach)))?;
    relay(attached).await?;
    Ok(())
}

/// The owner's welcomed connection for this process.
#[cfg(unix)]
fn attach(project_root: &Path) -> Result<Attached, AttachError> {
    let config = NativeConfig::from_env(project_root);
    let project_id = config.resolve_project_id();
    let (generation, stream) = OwnerClient::attach_mcp(
        &config,
        project_id,
        WriterRole::Service,
        CallerIdentity::this_process("mcp-serve"),
        ATTACH_DEADLINE,
    )?;
    tracing::info!(
        target: "magistr::mcp",
        generation = generation.get(),
        "attached to the running project owner"
    );
    Ok(stream)
}

/// No owner to attach to on this platform.
#[cfg(not(unix))]
fn attach(_project_root: &Path) -> Result<Attached, AttachError> {
    Err(AttachError::Unsupported)
}

/// Copy this process's stdio to and from the owner until one side ends.
#[cfg(unix)]
async fn relay(attached: Attached) -> io::Result<()> {
    ProxyStdio::relay(attached, "project").await
}

/// Unreachable: nothing is ever attached here.
#[cfg(not(unix))]
async fn relay(attached: Attached) -> io::Result<()> {
    match attached {}
}

/// Why this process could neither own the project nor attach to its owner.
fn attach_refusal(lease: &io::Error, attach: &AttachError) -> String {
    format!("{lease}\nmagistr: and the owner endpoint did not take this server: {attach}")
}
