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

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre;
use magistr_engine::tas::{
    AttachError, CallerIdentity, McpAcceptor, OwnerClient, OwnerEndpoint, TasConfig, WriterRole,
};
use rmcp::ServiceExt as _;
use think_and_ship::infra::resolve_project_id_with;

use super::owner_stdio::ProxyStdio;
use super::protocol_cap::ProtocolCap;
use super::service::MagistrService;

/// How long the handshake with the owner may take.
const ATTACH_DEADLINE: Duration = Duration::from_secs(5);

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
    pub async fn serve_owner_endpoint(&self) -> io::Result<OwnerEndpoint> {
        let service = self.clone();
        let acceptor: McpAcceptor = Arc::new(move |stream: tokio::net::UnixStream| {
            let service = service.clone();
            tokio::spawn(async move {
                let transport = ProtocolCap::new(rmcp::transport::IntoTransport::<
                    rmcp::service::RoleServer,
                    io::Error,
                    _,
                >::into_transport(stream));
                match service.serve(transport).await {
                    Ok(running) => {
                        let _ = running.waiting().await;
                    }
                    Err(error) => tracing::warn!(
                        target: "magistr::mcp",
                        "an attached MCP session did not start: {error}"
                    ),
                }
            });
        });
        OwnerEndpoint::serve_mcp(Arc::clone(&self.authority), acceptor).await
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
pub async fn proxy_to_owner(project_root: &Path, refusal: &io::Error) -> eyre::Result<()> {
    let config = TasConfig::from_env(project_root);
    let (project_id, _) = resolve_project_id_with(None, Some(config.project_root()));
    let (generation, stream) = OwnerClient::attach_mcp(
        &config,
        project_id,
        WriterRole::Service,
        CallerIdentity::this_process("mcp-serve"),
        ATTACH_DEADLINE,
    )
    .map_err(|attach| eyre::eyre!("{}", attach_refusal(refusal, &attach)))?;
    tracing::info!(
        target: "magistr::mcp",
        generation = generation.get(),
        "attached to the running project owner"
    );
    stream.set_nonblocking(true)?;
    let mut owner = tokio::net::UnixStream::from_std(stream)?;
    let (mut from_owner, mut to_owner) = owner.split();
    let mut stdin = ProxyStdio::stdin()?;
    let mut stdout = ProxyStdio::stdout()?;
    tokio::select! {
        client = tokio::io::copy(&mut stdin, &mut to_owner) => {
            client?;
            Ok(())
        }
        owner = tokio::io::copy(&mut from_owner, &mut stdout) => {
            owner?;
            Err(eyre::eyre!(
                "magistr: the project owner ended its session; start a new server to take \
                 the project"
            ))
        }
    }
}

/// Why this process could neither own the project nor attach to its owner.
fn attach_refusal(lease: &io::Error, attach: &AttachError) -> String {
    format!("{lease}\nmagistr: and the owner endpoint did not take this server: {attach}")
}
