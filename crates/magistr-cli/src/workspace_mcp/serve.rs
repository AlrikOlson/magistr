use std::{path::Path, sync::Arc};

use color_eyre::eyre;
use magistr_workspace::{
    commands::ScriptInterpreter,
    config::SessionFlags,
    service::{WorkspaceOpenOptions, WorkspaceService},
};
use rmcp::{ServiceExt, service::ServerInitializeError};

use super::{WorkspaceMcpService, connection::WorkspaceConnection};
use crate::mcp_tools::protocol_cap::ProtocolCap;

/// Pinned interpreter embedded in the shipping binary.
const QJS_WASI: &[u8] =
    include_bytes!("../../../../subsystems/workspace/reference/vendor/qjs/qjs-wasi.wasm");

/// Serve one native workspace, or attach this stdio client to its admitted owner.
///
/// The first invocation owns the service for its lifetime. Additional clients
/// use the existing versioned local owner protocol and open no workspace stores.
/// Owner loss ends attached transports; resilient reconnect belongs to the
/// subsequent lifecycle integration, before production launcher cutover.
pub async fn run_workspace_stdio_server(root: &Path, config: &[String]) -> eyre::Result<()> {
    let flags = SessionFlags::parse(config);
    if !flags.errors.is_empty() {
        fail(64, &flags.errors.join("; "));
    }
    let options = WorkspaceOpenOptions::from_environment(root, std::env::current_exe()?)
        .with_session_flags(flags.flags)
        .with_interpreter(ScriptInterpreter::from_bytes(QJS_WASI.to_vec()));
    let connection = match WorkspaceConnection::open(options).await {
        Ok(connection) => connection,
        Err(error) => fail(error.exit_code(), &error.to_string()),
    };
    let service = match connection {
        WorkspaceConnection::Owner(service) => service,
        #[cfg(unix)]
        WorkspaceConnection::Attached(stream) => {
            crate::mcp_tools::owner_stdio::ProxyStdio::relay(stream.into_std()?, "workspace")
                .await?;
            return Ok(());
        }
    };
    serve_owner(service).await
}

async fn serve_owner(service: Arc<WorkspaceService>) -> eyre::Result<()> {
    #[cfg(unix)]
    let endpoint = match super::endpoint::serve(Arc::clone(&service)).await {
        Ok(endpoint) => endpoint,
        Err(error) => {
            shutdown(service).await?;
            return Err(error.into());
        }
    };
    let result = async {
        let handler = WorkspaceMcpService::new(Arc::clone(&service))?;
        let complete = handler.completion();
        // Preserve the protocol cap already used by workflow and workspace MCP.
        let transport = ProtocolCap::new(rmcp::transport::IntoTransport::<
            rmcp::service::RoleServer,
            std::io::Error,
            _,
        >::into_transport(rmcp::transport::stdio()));
        let result = match handler.serve(transport).await {
            Ok(running) => running
                .waiting()
                .await
                .map(|_| ())
                .map_err(eyre::Report::from),
            Err(ServerInitializeError::ConnectionClosed(_)) => Ok(()),
            Err(error) => Err(eyre::Report::from(error)),
        };
        complete.cancelled().await;
        result
    }
    .await;
    #[cfg(unix)]
    endpoint.shutdown().await?;
    shutdown(service).await?;
    result
}

async fn shutdown(service: Arc<WorkspaceService>) -> eyre::Result<()> {
    let service = Arc::try_unwrap(service).map_err(|_| {
        eyre::eyre!("workspace service references remain after all MCP handlers stopped; explicit service shutdown refused")
    })?;
    service.shutdown().await?;
    Ok(())
}

fn fail(code: i32, message: &str) -> ! {
    eprintln!("magistr workspace: {message}");
    std::process::exit(code)
}
