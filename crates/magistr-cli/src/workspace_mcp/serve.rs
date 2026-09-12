use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::{
    commands::ScriptInterpreter,
    config::SessionFlags,
    service::{WorkspaceOpenOptions, mint_session_identity},
};

use super::{bridge::Bridge, bridge_link::Link};

/// Pinned interpreter embedded in the shipping binary.
const QJS_WASI: &[u8] =
    include_bytes!("../../../../subsystems/workspace/reference/vendor/qjs/qjs-wasi.wasm");

/// The launcher may name the session identity a bridge stamps; otherwise one
/// is minted for the life of this process.
const SESSION_ENV: &str = "WORKBENCH_SESSION_ID";

/// Serve one native workspace to this process's stdio client through the
/// bridge: the first invocation admits and owns the service, later ones attach
/// to it over the shared owner endpoint, and any of them survives the owner
/// going away by reconnecting on the next request.
pub async fn run_workspace_stdio_server(root: &Path, config: &[String]) -> eyre::Result<()> {
    let flags = SessionFlags::parse(config);
    if !flags.errors.is_empty() {
        fail(64, &flags.errors.join("; "));
    }
    let options = open_options(root, flags.flags)?;
    let link = match Link::open(&options).await {
        Ok(link) => link,
        Err(error) => fail(error.exit_code(), &error.to_string()),
    };
    let identity = match std::env::var(SESSION_ENV) {
        Ok(named) if !named.trim().is_empty() => named.trim().to_owned(),
        _ => mint_session_identity("bridge").map_err(|error| eyre::eyre!("{error}"))?,
    };
    Bridge::new(options, identity, link).run().await
}

/// The open options every Magistr owner of `root` composes — the bridge
/// and the run host alike: this binary as the worker, the host
/// environment's configuration layers, the embedded interpreter, plus the
/// session's `--config` flags.
pub(crate) fn open_options(
    root: &Path,
    flags: serde_json::Map<String, serde_json::Value>,
) -> std::io::Result<WorkspaceOpenOptions> {
    Ok(
        WorkspaceOpenOptions::from_environment(root, std::env::current_exe()?)
            .with_session_flags(flags)
            .with_interpreter(ScriptInterpreter::from_bytes(QJS_WASI.to_vec())),
    )
}

fn fail(code: i32, message: &str) -> ! {
    eprintln!("magistr workspace: {message}");
    std::process::exit(code)
}
