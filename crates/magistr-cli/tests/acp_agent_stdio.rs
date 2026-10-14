//! Magistr's own ACP client against magistr's own ACP agent, as a real
//! subprocess over stdio: `AcpTransport` spawns `magistr acp-agent
//! --provider scripted` — the bridge both halves of which are Rust — and
//! completes a turn, then cancels one.

#[path = "support/in_env.rs"]
mod in_env;

use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::v1::{SessionUpdate, StopReason};
use magistr_acp::{AcpEvent, AcpTransport};
use tokio::sync::mpsc;

type Events = mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>;

const WAIT: Duration = Duration::from_secs(30);

/// The magistr binary Cargo built for this test run.
fn magistr_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_magistr"))
}

async fn next(events: &mut Events) -> AcpEvent {
    tokio::time::timeout(WAIT, events.recv())
        .await
        .expect("an event before the timeout")
        .expect("channel open")
        .expect("no transport error")
}

/// Drain until an event matching `pred` arrives, returning it.
async fn next_matching(events: &mut Events, pred: impl Fn(&AcpEvent) -> bool) -> AcpEvent {
    loop {
        let event = next(events).await;
        if pred(&event) {
            return event;
        }
    }
}

fn message_text(event: &AcpEvent) -> Option<String> {
    match event {
        AcpEvent::SessionUpdate { update, .. } => match update.as_ref() {
            SessionUpdate::AgentMessageChunk(chunk) => {
                serde_json::to_value(&chunk.content).ok()?["text"]
                    .as_str()
                    .map(str::to_owned)
            }
            _ => None,
        },
        _ => None,
    }
}

#[tokio::test]
async fn the_rust_client_completes_a_turn_and_a_cancel_against_the_in_binary_agent() {
    if !in_env::in_env(&[
        ("MAGISTR_LAYOUT", None),
        ("MAGISTR_BUDGET", None),
        ("MAGISTR_LOG", None),
        ("MAGISTR_LOG_FILE", None),
        ("MAGISTR_TRUST", None),
    ]) {
        return;
    }
    let cwd = tempfile::tempdir().expect("tempdir");
    let mut transport = AcpTransport::new_with_args(
        magistr_binary(),
        vec![
            "acp-agent".to_string(),
            "--provider".to_string(),
            "scripted".to_string(),
            // What the runtime appends for every pre-approved tool.
            "--allow-tool=Bash".to_string(),
        ],
        Some(cwd.path().to_path_buf()),
    )
    .expect("construct");
    transport
        .connect()
        .await
        .expect("handshake with magistr acp-agent");
    assert!(transport.child_pgid() > 0, "the agent is a real subprocess");
    let mut events = transport.read_events().expect("events");

    // The handshake: our session id, the model picker, the provider in _meta.
    match next(&mut events).await {
        AcpEvent::Initialized {
            session_id,
            config_options,
            agent_capabilities,
            agent_meta,
        } => {
            assert!(
                session_id.len() == 36 && session_id.matches('-').count() == 4,
                "a UUID the runtime can be handed back: {session_id}"
            );
            assert!(agent_capabilities.load_session);
            let picker = config_options.expect("the model picker");
            assert_eq!(picker[0].id.to_string(), "model");
            let meta = agent_meta.expect("_meta.magistr");
            assert_eq!(meta["magistr"]["provider"], "scripted");
        }
        other => panic!("expected Initialized, got {other:?}"),
    }

    // A full turn.
    transport
        .prompt(&"hello over stdio".into())
        .expect("prompt");
    let mut text = String::new();
    loop {
        let event = next(&mut events).await;
        if let Some(chunk) = message_text(&event) {
            text.push_str(&chunk);
        }
        if let AcpEvent::PromptResponse { response, .. } = event {
            assert_eq!(response.stop_reason, StopReason::EndTurn);
            break;
        }
    }
    assert_eq!(text, "[scripted-fast] echo: hello over stdio");

    // A cancel mid-turn.
    transport.prompt(&"hang".into()).expect("prompt");
    next_matching(&mut events, |e| {
        message_text(e).is_some_and(|t| t.contains("hanging"))
    })
    .await;
    transport.cancel().expect("cancel");
    let ended = next_matching(&mut events, |e| {
        matches!(e, AcpEvent::PromptResponse { .. })
    })
    .await;
    let AcpEvent::PromptResponse { response, .. } = ended else {
        unreachable!()
    };
    assert_eq!(response.stop_reason, StopReason::Cancelled);

    transport.close().await.expect("close");
}
