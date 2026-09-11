use std::{path::Path, sync::Arc};

use color_eyre::eyre;
use magistr_workspace::{
    commands::ScriptInterpreter,
    config::SessionFlags,
    service::{WorkspaceOpenOptions, WorkspaceService},
};
use rmcp::{ServiceExt, service::ServerInitializeError};

use super::WorkspaceMcpService;
use crate::mcp_tools::protocol_cap::ProtocolCap;

/// The retained QuickJS-ng interpreter the script tool runs: the pinned
/// artifact embedded at build time, so the shipping binary needs no
/// reference tree, Bun or Node beside it at runtime.
const QJS_WASI: &[u8] =
    include_bytes!("../../../../subsystems/workspace/reference/vendor/qjs/qjs-wasi.wasm");

/// EX_USAGE, as the reference answered a malformed `--config`.
const EXIT_USAGE: i32 = 64;

/// Serve MCP over stdio for one workspace until the client goes away.
///
/// stdout belongs to the transport; every diagnostic goes to stderr. A
/// refusal before the transport exists is a process-level event with the
/// retained exit code (64 usage, 65 durable data this binary cannot read,
/// 66 missing root, 75 another live instance owns the workspace), not an
/// envelope, because no negotiated request is in flight yet. The worker
/// role is this same binary.
pub async fn run_workspace_stdio_server(root: &Path, config: &[String]) -> eyre::Result<()> {
    let flags = SessionFlags::parse(config);
    if !flags.errors.is_empty() {
        fail(EXIT_USAGE, &flags.errors.join("; "));
    }
    let executable = std::env::current_exe()?;
    let options = WorkspaceOpenOptions::from_environment(root, executable)
        .with_session_flags(flags.flags)
        .with_interpreter(ScriptInterpreter::from_bytes(QJS_WASI.to_vec()));
    let service = match WorkspaceService::open(options).await {
        Ok(service) => Arc::new(service),
        Err(error) => fail(error.exit_code(), &error.to_string()),
    };
    let handler = WorkspaceMcpService::new(Arc::clone(&service))?;
    // Capped below 2026-07-28 like `mcp-serve`: the SEP-2322 result shapes of
    // that era are what the Claude Agent SDK's bundled CLI rejects.
    let transport = ProtocolCap::new(rmcp::transport::IntoTransport::<
        rmcp::service::RoleServer,
        std::io::Error,
        _,
    >::into_transport(rmcp::transport::stdio()));
    // stdin reaching EOF closes the transport: the client is gone, whether
    // it left after a session or before ever initializing. Either way the
    // workspace is released the way a signal would release it, so the next
    // session is not refused with 75.
    match handler.serve(transport).await {
        Ok(running) => {
            running.waiting().await?;
        }
        Err(ServerInitializeError::ConnectionClosed(_)) => {}
        Err(error) => return Err(error.into()),
    }
    if let Ok(service) = Arc::try_unwrap(service) {
        service.shutdown().await?;
    }
    Ok(())
}

fn fail(code: i32, message: &str) -> ! {
    eprintln!("magistr workspace: {message}");
    std::process::exit(code)
}
