use color_eyre::eyre;
use magistr_workspace::{
    commands::ScriptInterpreter,
    config::SessionFlags,
    service::{WorkspaceOpenOptions, mint_session_identity},
};

use super::{bridge::Bridge, bridge_link::Link};
use crate::config::inputs::ProjectInputs;

/// Pinned interpreter embedded in the shipping binary.
const QJS_WASI: &[u8] =
    include_bytes!("../../../../subsystems/workspace/reference/vendor/qjs/qjs-wasi.wasm");

/// Serve one native workspace — the project the captured inputs are rooted
/// at — to this process's stdio client through the bridge: the first
/// invocation admits and owns the service, later ones attach to it over the
/// shared owner endpoint, and any of them survives the owner going away by
/// reconnecting on the next request. The launcher may name the session
/// identity the bridge stamps (the captured `WORKBENCH_SESSION_ID`);
/// otherwise one is minted for the life of this process.
pub async fn run_workspace_stdio_server(
    inputs: &ProjectInputs,
    config: &[String],
) -> eyre::Result<()> {
    let flags = SessionFlags::parse(config);
    if !flags.errors.is_empty() {
        fail(64, &flags.errors.join("; "));
    }
    let options = open_options(inputs, flags.flags)?;
    let link = match Link::open(&options).await {
        Ok(link) => link,
        Err(error) => fail(error.exit_code(), &error.to_string()),
    };
    let identity = match inputs.workspace_session_id() {
        Some(named) => named.to_owned(),
        None => mint_session_identity("bridge").map_err(|error| eyre::eyre!("{error}"))?,
    };
    Bridge::new(options, identity, link).run().await
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
