//! Explicit live acceptance check for the common Codex connection path.
//!
//! MAGISTR_CODEX_LIVE=1 cargo run --example codex_transport_live
//!
//! Requires an installed Codex CLI with a ChatGPT login. Spends one short
//! gpt-5.6-sol turn in a disposable directory; never runs in the checkout.

use std::path::Path;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    SessionConfigOptionValue, SessionUpdate, StopReason, ToolCallStatus, ToolKind,
};
use color_eyre::eyre::{self, ensure};
use magistr::agent::{AgentConfig, connect_transport};
use magistr_acp::{
    AcpEvent, AcpPermissionOptionKind, AcpPermissionRequest, AcpPermissionResponse, AcpTransport,
};

const MODEL: &str = "gpt-5.6-sol";
const COMMAND: &str = "echo codex-live-ok > codex-live-proof.txt";
const WAIT: Duration = Duration::from_secs(180);

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // AgentRuntime::in_binary resolves current_exe. This example therefore
    // dispatches its child invocation through the same production command.
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "acp-agent") {
        ensure!(args.get(1).map(String::as_str) == Some("--provider"));
        ensure!(args.get(2).map(String::as_str) == Some("codex"));
        let allowed: Vec<_> = args
            .iter()
            .skip(3)
            .filter_map(|arg| arg.strip_prefix("--allow-tool=").map(str::to_owned))
            .collect();
        return magistr::commands::misc::cmd_acp_agent("codex", &allowed).await;
    }
    ensure!(
        std::env::var("MAGISTR_CODEX_LIVE").as_deref() == Ok("1"),
        "Set MAGISTR_CODEX_LIVE=1 to authorize this live subscription check"
    );
    let project = tempfile::tempdir()?;
    let cwd = project.path().canonicalize()?;
    let config = AgentConfig::new("", &cwd)
        .provider("codex")
        .allowed_tools(Vec::<String>::new());
    ensure!(config.resolve_runtime()?.provider_id() == "codex");
    let mut transport = tokio::time::timeout(WAIT, connect_transport(&config)).await??;
    #[cfg(unix)]
    let pgid = transport.child_pgid();
    let result = tokio::time::timeout(WAIT, exercise(&mut transport, &cwd)).await;
    let closed = transport.close().await;
    result??;
    closed?;
    #[cfg(unix)]
    {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                // SAFETY: signal 0 only probes the disposable child's process group.
                if unsafe { libc::killpg(pgid, 0) } != 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .map_err(|_| eyre::eyre!("child process group survived close"))?;
    }
    eprintln!(
        "PASS: Codex via connect_transport; {MODEL}; approved marker write; streamed completed tool; EndTurn; exact file; child closed"
    );
    Ok(())
}

async fn exercise(transport: &mut AcpTransport, cwd: &Path) -> eyre::Result<()> {
    let mut events = transport
        .read_events()
        .ok_or_else(|| eyre::eyre!("missing events"))?;
    let mut permissions = transport
        .take_permission_rx()
        .ok_or_else(|| eyre::eyre!("missing permission channel"))?;
    let session_id = loop {
        let event = events
            .recv()
            .await
            .ok_or_else(|| eyre::eyre!("EOF before initialize"))??;
        if let AcpEvent::Initialized {
            session_id,
            agent_meta,
            ..
        } = event
        {
            ensure!(!session_id.is_empty(), "missing runtime thread identity");
            ensure!(
                agent_meta
                    .as_ref()
                    .is_some_and(|meta| meta["magistr"]["provider"] == "codex"),
                "provider identity mismatch"
            );
            break session_id;
        }
    };
    transport
        .set_config_option(
            session_id.clone(),
            "model".into(),
            SessionConfigOptionValue::ValueId {
                value: MODEL.into(),
            },
        )
        .await?;
    transport.prompt(&format!(
        "In the current directory run exactly this shell command once: {COMMAND}.          Use your command execution tool. Do not use apply_patch or any other tool.          Do not inspect other files or call MCP servers. Then reply with exactly: verified"
    ))?;
    let mut approved_tool: Option<String> = None;
    let mut completed_tool = false;
    let mut streamed = false;
    loop {
        tokio::select! {
            request = permissions.recv() => {
                let request = request.ok_or_else(|| eyre::eyre!("permission channel closed"))?;
                let choice = if approved_tool.is_none() && permitted(&request, cwd) {
                    request.options.iter()
                        .find(|option| option.kind == AcpPermissionOptionKind::AllowOnce)
                        .map(|option| option.option_id.clone())
                } else { None };
                let selected = choice.is_some();
                let answer = choice.map(AcpPermissionResponse::Selected)
                    .unwrap_or(AcpPermissionResponse::Cancelled);
                request.response_tx.send(answer)
                    .map_err(|_| eyre::eyre!("permission receiver closed before decision"))?;
                if selected { approved_tool = Some(request.tool_call_id); }
            }
            event = events.recv() => {
                match event.ok_or_else(|| eyre::eyre!("EOF before terminal turn"))?? {
                    AcpEvent::SessionUpdate { update, session_id: actual } => {
                        ensure!(actual == session_id, "cross-session event");
                        match update.as_ref() {
                            SessionUpdate::AgentMessageChunk(_) => streamed = true,
                            SessionUpdate::ToolCallUpdate(update)
                                if update.fields.status == Some(ToolCallStatus::Completed)
                                    && approved_tool.as_deref() == Some(update.tool_call_id.0.as_ref()) =>
                                    completed_tool = true,
                            _ => {}
                        }
                    }
                    AcpEvent::PromptResponse { response, session_id: actual } => {
                        ensure!(actual == session_id, "cross-session terminal");
                        ensure!(response.stop_reason == StopReason::EndTurn, "turn did not complete");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    ensure!(
        approved_tool.is_some(),
        "expected marker command did not traverse permission relay"
    );
    ensure!(
        completed_tool && streamed,
        "missing completed tool or streamed message"
    );
    ensure!(
        std::fs::read_to_string(cwd.join("codex-live-proof.txt"))? == "codex-live-ok\n",
        "marker file did not match"
    );
    Ok(())
}

fn permitted(request: &AcpPermissionRequest, cwd: &Path) -> bool {
    let input = &request.tool_input;
    let expected = [
        format!("/bin/zsh -lc '{COMMAND}'"),
        format!("/bin/bash -lc '{COMMAND}'"),
    ];
    request.tool_kind == Some(ToolKind::Execute)
        && input["cwd"]
            .as_str()
            .is_some_and(|value| Path::new(value) == cwd)
        && input["command"]
            .as_str()
            .is_some_and(|value| expected.iter().any(|command| command == value))
}
