//! Live smoke test: magistr's own client against magistr's own agent driving
//! the real `claude` on PATH — the bridge people actually run.
//!
//! Ignored by default — it needs `claude` installed and logged in, and it
//! spends a few real turns. Run it with:
//!
//! ```bash
//! MAGISTR_ACP_LIVE=1 cargo nextest run -p magistr-cli --run-ignored ignored-only -E 'binary(acp_live)'
//! ```
//!
//! This is the check a green build cannot give: the provider's fixtures pin
//! the wire as it was recorded, and this pins it as it is today.

use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    McpServer, McpServerStdio, SessionUpdate, StopReason, ToolCallStatus,
};
use magistr_acp::{AcpEvent, AcpPermissionResponse, AcpTransport};

const TURN_TIMEOUT: Duration = Duration::from_secs(180);

fn live() -> bool {
    std::env::var("MAGISTR_ACP_LIVE").is_ok_and(|v| !v.is_empty())
}

/// The magistr binary Cargo built for this test run.
fn magistr_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_magistr"))
}

#[tokio::test]
#[ignore = "needs a real, logged-in claude on PATH; set MAGISTR_ACP_LIVE=1"]
async fn the_in_binary_claude_agent_advertises_fable_runs_a_tool_turn_and_cancels_the_next() {
    if !live() {
        eprintln!("MAGISTR_ACP_LIVE not set; skipping");
        return;
    }
    let cwd = tempfile::tempdir().expect("tempdir");
    let mut transport = AcpTransport::new_with_args(
        magistr_binary(),
        vec![
            "acp-agent".to_string(),
            "--provider".to_string(),
            "claude".to_string(),
        ],
        Some(cwd.path().to_path_buf()),
    )
    .expect("construct")
    .mcp_servers(vec![McpServer::Stdio(
        McpServerStdio::new("magistr", magistr_binary()).args(vec!["mcp".to_string()]),
    )]);
    transport
        .connect()
        .await
        .expect("handshake with the in-binary claude agent");
    assert!(
        transport.child_pgid() > 0,
        "the agent runs in its own process group"
    );

    // Auto-approve anything the agent asks for; this is a sandboxed temp dir.
    let mut perm_rx = transport.take_permission_rx().expect("permission channel");
    tokio::spawn(async move {
        while let Some(req) = perm_rx.recv().await {
            let first = req.options.first().map(|o| o.option_id.clone());
            let _ = req.response_tx.send(match first {
                Some(id) => AcpPermissionResponse::Selected(id),
                None => AcpPermissionResponse::Cancelled,
            });
        }
    });
    let mut events = transport.read_events().expect("events");

    // 1. The handshake carries the model picker, and Fable is on it. The
    //    agent replays the CLI's command list on session/new, so updates may
    //    precede the Initialized event.
    let init = tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            match events
                .recv()
                .await
                .expect("channel open")
                .expect("no transport error")
            {
                AcpEvent::SessionUpdate { .. } => {}
                other => return other,
            }
        }
    })
    .await
    .expect("initialized in time");
    let AcpEvent::Initialized {
        config_options,
        agent_meta,
        ..
    } = init
    else {
        panic!("the handshake must end in Initialized, got {init:?}");
    };
    let json = serde_json::to_string(&config_options).expect("serialize");
    assert!(json.contains("claude-fable-5"), "model picker: {json}");
    assert_eq!(
        agent_meta.expect("_meta.magistr")["magistr"]["provider"],
        "claude"
    );

    // 2. A real turn streams and ends.
    transport
        .prompt(&"Reply with exactly the word: pong".into())
        .expect("prompt");
    let mut saw_chunk = false;
    let stop = tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            match events.recv().await.expect("channel open") {
                Ok(AcpEvent::SessionUpdate { .. }) => saw_chunk = true,
                Ok(AcpEvent::PromptResponse { response, .. }) => return response.stop_reason,
                Ok(_) => {}
                Err(e) => panic!("transport error mid-turn: {e}"),
            }
        }
    })
    .await
    .expect("turn finished in time");
    assert_eq!(stop, StopReason::EndTurn);
    assert!(
        saw_chunk,
        "the turn should have streamed at least one update"
    );

    // 3. The MCP protocol cap holds: `magistr mcp` is reachable as a
    //    tool under the direct CLI, and the call round-trips permission.
    transport
        .prompt(
            &"Call the mcp__magistr__search tool to list files (mode \"files\", query \"*\"), \
             then reply with exactly the word: listed"
                .into(),
        )
        .expect("prompt");
    let mut magistr_call = false;
    let stop = tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            match events.recv().await.expect("channel open") {
                Ok(AcpEvent::SessionUpdate { update, .. }) => match update.as_ref() {
                    SessionUpdate::ToolCall(call) if call.title.contains("mcp__magistr__") => {
                        magistr_call = true;
                    }
                    SessionUpdate::ToolCallUpdate(u)
                        if u.fields.status == Some(ToolCallStatus::Completed) => {}
                    _ => {}
                },
                Ok(AcpEvent::PromptResponse { response, .. }) => return response.stop_reason,
                Ok(_) => {}
                Err(e) => panic!("transport error mid-turn: {e}"),
            }
        }
    })
    .await
    .expect("tool turn finished in time");
    assert_eq!(stop, StopReason::EndTurn);
    assert!(
        magistr_call,
        "the model should have called an mcp__magistr__ tool"
    );

    // 4. Cancel mid-turn is honoured.
    transport
        .prompt(&"Count slowly from 1 to 200, one number per line.".into())
        .expect("third prompt");
    tokio::time::sleep(Duration::from_secs(3)).await;
    transport.cancel().expect("cancel");
    let stop = tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            match events.recv().await.expect("channel open") {
                Ok(AcpEvent::PromptResponse { response, .. }) => return response.stop_reason,
                Ok(_) => {}
                Err(e) => panic!("transport error after cancel: {e}"),
            }
        }
    })
    .await
    .expect("cancelled turn finished in time");
    assert_eq!(stop, StopReason::Cancelled);

    // 5. A turn after the cancel runs fresh.
    transport
        .prompt(&"Reply with exactly the word: again".into())
        .expect("fourth prompt");
    let stop = tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            match events.recv().await.expect("channel open") {
                Ok(AcpEvent::PromptResponse { response, .. }) => return response.stop_reason,
                Ok(_) => {}
                Err(e) => panic!("transport error after cancel: {e}"),
            }
        }
    })
    .await
    .expect("fresh turn finished in time");
    assert_eq!(stop, StopReason::EndTurn);

    #[cfg(unix)]
    let pgid = transport.child_pgid();
    transport.close().await.expect("close");
    // The agent is gone.
    tokio::time::sleep(Duration::from_millis(500)).await;
    #[cfg(unix)]
    {
        // SAFETY: kill(pgid, 0) only probes for existence.
        let alive = unsafe { libc::killpg(pgid, 0) } == 0;
        assert!(
            !alive,
            "agent process group {pgid} should be dead after close"
        );
    }
}
