//! The cockpit's path to an API provider, with no code path of its own:
//! `AcpTransport` spawns `magistr acp-agent --provider openai` — the same
//! binary, the same arguments the runtime builds — and completes a turn
//! against a fake OpenAI server the agent reaches through `OPENAI_BASE_URL`.
//!
//! The fake server stays in the parent while a test child starts with
//! explicit credentials and endpoint settings inherited by the ACP agent.

#[path = "../../magistr-acp-agent/tests/fake_http/mod.rs"]
mod fake_http;
#[path = "support/in_env.rs"]
mod in_env;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::schema::v1::{SessionUpdate, StopReason};
use fake_http::{FakeHttp, Reply};
use magistr_acp::{AcpEvent, AcpSessionMode, AcpTransport};
use tokio::sync::mpsc;

type Events = mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>;

const WAIT: Duration = Duration::from_secs(30);

fn fixture(path: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/crates/magistr-acp-agent/tests/fixtures/{path}",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../..")
    ))
    .expect("the fixture exists")
}

async fn next(events: &mut Events) -> AcpEvent {
    tokio::time::timeout(WAIT, events.recv())
        .await
        .expect("an event before the timeout")
        .expect("channel open")
        .expect("no transport error")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_cockpits_transport_runs_a_turn_on_the_openai_provider() {
    let config_home = tempfile::tempdir().expect("isolated config home");
    let server = FakeHttp::serve(vec![Reply::stream(
        "text/event-stream",
        &fixture("openai/turn.sse"),
    )])
    .await;
    // The parent runtime drives this HTTP server while the test child runs
    // the real ACP transport with the explicitly inherited endpoint.
    if !in_env::in_env(&[
        ("OPENAI_API_KEY", Some(OsStr::new("sk-test-stdio"))),
        ("OPENAI_BASE_URL", Some(OsStr::new(&server.url))),
        ("MAGISTR_LAYOUT", Some(OsStr::new("vertical"))),
        ("XDG_CONFIG_HOME", Some(config_home.path().as_os_str())),
    ]) {
        let requests = server.recorded();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].header("authorization"),
            Some("Bearer sk-test-stdio"),
            "the key reached the agent through its environment"
        );
        return;
    }

    let cwd = tempfile::tempdir().expect("tempdir");
    let config = magistr::agent::AgentConfig::new("why is the sky blue", cwd.path())
        .provider("openai")
        .apply_user_defaults();
    let runtime = config
        .resolve_runtime()
        .expect("the session's in-binary agent");
    assert_eq!(runtime.provider_id(), "openai");
    // The runtime resolves to the running test binary; the agent is the
    // magistr binary Cargo built for this run, with the runtime's arguments.
    let mut transport = AcpTransport::new_with_args(
        PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
        runtime.args.clone(),
        Some(cwd.path().to_path_buf()),
    )
    .expect("construct");
    transport
        .connect()
        .await
        .expect("handshake with magistr acp-agent --provider openai");
    let mut events = transport.read_events().expect("events");

    let session_id = match next(&mut events).await {
        AcpEvent::Initialized {
            config_options,
            agent_meta,
            agent_capabilities,
            session_id,
        } => {
            assert!(
                !agent_capabilities.load_session,
                "API history is process-local"
            );
            let picker = config_options.expect("the model picker");
            assert_eq!(picker[0].id.to_string(), "model");
            let meta = agent_meta.expect("_meta.magistr");
            assert_eq!(meta["magistr"]["provider"], "openai");
            assert_eq!(meta["magistr"]["models"][0]["id"], "gpt-4o");
            session_id
        }
        other => panic!("expected Initialized, got {other:?}"),
    };

    transport
        .prompt(&"why is the sky blue".into())
        .expect("prompt");
    let mut text = String::new();
    let mut cost = None;
    loop {
        let event = next(&mut events).await;
        if let AcpEvent::SessionUpdate { update, .. } = &event {
            match update.as_ref() {
                SessionUpdate::AgentMessageChunk(chunk) => {
                    if let Some(t) = serde_json::to_value(&chunk.content)
                        .ok()
                        .and_then(|v| v["text"].as_str().map(str::to_owned))
                    {
                        text.push_str(&t);
                    }
                }
                SessionUpdate::UsageUpdate(usage) => {
                    cost = usage.cost.as_ref().map(|c| c.amount);
                }
                _ => {}
            }
        }
        if let AcpEvent::PromptResponse { response, .. } = event {
            assert_eq!(response.stop_reason, StopReason::EndTurn);
            break;
        }
    }
    assert_eq!(text, "Rayleigh scattering.");
    let cost = cost.expect("the usage update carried the cost");
    assert!((cost - 3.50).abs() < 1e-9, "{cost}");

    transport.close().await.expect("close");

    // A different agent process cannot restore this API adapter's in-memory
    // history. The real CLI/transport must refuse, without issuing another HTTP turn.
    let mut resumed = AcpTransport::new_with_args(
        PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
        runtime.args.clone(),
        Some(cwd.path().to_path_buf()),
    )
    .expect("fresh transport")
    .session_mode(AcpSessionMode::Load {
        session_id: session_id.clone(),
    });
    let error = resumed
        .connect()
        .await
        .expect_err("API sessions cannot survive a process restart");
    assert!(error.to_string().contains("does not support"), "{error}");
    assert!(error.to_string().contains(&session_id), "{error}");
    assert!(!resumed.is_ready());
    assert!(
        resumed
            .prompt(&"do not start empty history".into())
            .is_err()
    );
}
