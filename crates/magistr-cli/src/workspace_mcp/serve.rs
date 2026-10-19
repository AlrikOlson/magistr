use color_eyre::eyre;
use magistr_workspace::{
    commands::ScriptInterpreter,
    config::SessionFlags,
    service::{WorkspaceOpenError, WorkspaceOpenOptions, mint_session_identity},
};

use super::{bridge_link::Link, capability::Capability};
use crate::config::inputs::ProjectInputs;
use crate::mcp_bridge::Bridge;

/// Pinned interpreter embedded in the shipping binary.
const QJS_WASI: &[u8] = include_bytes!("../../assets/qjs/qjs-wasi.wasm");

/// Internal workspace transport admitted by the unified MCP endpoint.
pub(crate) async fn run_transport(
    inputs: &ProjectInputs,
    config: &[String],
    input: impl tokio::io::AsyncRead + Unpin,
    output: impl tokio::io::AsyncWrite + Unpin,
) -> eyre::Result<()> {
    let flags = SessionFlags::parse(config);
    if !flags.errors.is_empty() {
        fail(64, &flags.errors.join("; "));
    }
    let options = open_options(inputs, flags.flags)?;
    let link = match Link::open(&options).await {
        Ok(link) => Ok(link),
        Err(error) if error.exit_code() == WorkspaceOpenError::EXIT_LOCKED => {
            eprintln!("magistr workspace: {error}; serving discovery until an owner is reachable");
            Err(error.to_string())
        }
        Err(error) => fail(error.exit_code(), &error.to_string()),
    };
    let identity = match inputs.workspace_session_id() {
        Some(named) => named.to_owned(),
        None => mint_session_identity("bridge").map_err(|error| eyre::eyre!("{error}"))?,
    };
    let capability = Capability::new(options.offers_script());
    Bridge::new(options, identity, capability, link)
        .run_transport(input, output)
        .await
}

/// The open options every Magistr owner of the captured project composes —
/// the bridge and the run host alike: this binary as the worker, the
/// captured configuration layers and network override, the embedded
/// interpreter, plus the session's `--config` flags.
pub(crate) fn open_options(
    inputs: &ProjectInputs,
    flags: serde_json::Map<String, serde_json::Value>,
) -> std::io::Result<WorkspaceOpenOptions> {
    let mut options = WorkspaceOpenOptions::new(inputs.project_root(), std::env::current_exe()?);
    options.env = inputs.workspace_env().clone();
    options.network_override = inputs.workspace_network_override().map(str::to_owned);
    Ok(options
        .with_session_flags(flags)
        .with_interpreter(ScriptInterpreter::from_bytes(QJS_WASI.to_vec())))
}

fn fail(code: i32, message: &str) -> ! {
    eprintln!("magistr workspace: {message}");
    std::process::exit(code)
}
