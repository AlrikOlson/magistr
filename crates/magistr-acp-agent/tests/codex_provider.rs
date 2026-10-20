//! Codex App Server through the real ACP session, permission and cancel paths.
//! Happy paths replay CLI 0.153.4 recordings. Fault tests explicitly modify a
//! temporary copy; their injected failures are not represented as recordings.

mod fake_client;
#[path = "../../magistr-cli/tests/support/in_env.rs"]
mod in_env;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agent_client_protocol::BoxFuture;
use agent_client_protocol::schema::v1::{
    CancelNotification, EnvVariable, LoadSessionRequest, McpServer, McpServerStdio,
    NewSessionRequest, PromptResponse, SessionConfigOptionValue, SessionId, SessionUpdate,
    SetSessionConfigOptionRequest, StopReason, ToolCallStatus,
};
use fake_client::{
    PermissionAnswer, initialize, new_session, prompt, prompt_and_wait, with_provider,
};
use magistr_acp_agent::{
    AgentError, CodexProvider, ModelChoice, Provider, SessionOpened, SessionStart, Turn,
};
use serde_json::{Value, json};

const THREAD: &str = "00000000-0000-4000-8000-000000000103";
const INTERRUPT_TURN: &str = "00000000-0000-4000-8000-00000000010d";

/// Retain a direct handle for steer/read while ACP owns a provider handle.
#[derive(Clone)]
struct Shared(Arc<CodexProvider>);

impl Provider for Shared {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn models(&self) -> Vec<ModelChoice> {
        self.0.models()
    }

    fn capabilities(&self) -> magistr_acp_agent::ProviderCapabilities {
        self.0.capabilities()
    }
    fn requires_terminal_cancellation(&self) -> bool {
        self.0.requires_terminal_cancellation()
    }

    fn start(&self, session: SessionStart) -> BoxFuture<'_, Result<SessionOpened, AgentError>> {
        self.0.start(session)
    }

    fn prompt(&self, turn: Turn) -> BoxFuture<'_, Result<StopReason, AgentError>> {
        self.0.prompt(turn)
    }
}

struct Recording {
    dir: tempfile::TempDir,
    frames: Vec<Value>,
}

impl Recording {
    fn new(name: &str) -> Self {
        let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/codex")
            .join(format!("{name}.ndjson"));
        Self {
            dir: tempfile::tempdir().expect("recording directory"),
            frames: std::fs::read_to_string(file)
                .expect("recorded fixture")
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| serde_json::from_str(line).expect("recorded JSON"))
                .collect(),
        }
    }

    fn marker(&self, method: &str) -> usize {
        self.frames
            .iter()
            .position(|frame| frame["_sent"]["method"] == method)
            .expect("the recording sent this method")
    }

    fn response(&self, method: &str) -> usize {
        let at = self.marker(method);
        let id = &self.frames[at]["_sent"]["id"];
        (at + 1..self.frames.len())
            .find(|&i| {
                let frame = &self.frames[i];
                frame.get("method").is_none() && frame.get("id") == Some(id)
            })
            .expect("the recording answered this request")
    }

    async fn discover(&self) -> Result<CodexProvider, AgentError> {
        let path = self.dir.path().join("replay.ndjson");
        let text = self
            .frames
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, text).expect("write temporary replay");
        CodexProvider::discover_with(
            PathBuf::from(env!("CARGO_BIN_EXE_fake_codex")),
            vec![
                path.to_string_lossy().into_owned(),
                "--input-log".into(),
                self.dir
                    .path()
                    .join("input.ndjson")
                    .to_string_lossy()
                    .into_owned(),
            ],
        )
        .await
    }

    async fn provider(&self) -> Shared {
        Shared(Arc::new(
            self.discover()
                .await
                .expect("recorded Codex discovery")
                .effort("low"),
        ))
    }

    fn inputs(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("input.ndjson"))
            .expect("flushed input log")
            .lines()
            .map(|line| serde_json::from_str(line).expect("logged JSON"))
            .collect()
    }

    fn request(&self, method: &str) -> Value {
        self.inputs()
            .into_iter()
            .find(|frame| frame["method"] == method)
            .expect("provider sent this method")
    }
}

#[tokio::test]
async fn discovery_advertises_the_runtime_catalog_and_completes_the_handshake_in_order() {
    let recording = Recording::new("turn_message");
    let provider = recording.provider().await;
    assert_eq!(provider.name(), "codex");
    let models = provider.models();
    assert_eq!(models[0].id, "gpt-6-astra");
    assert!(models.iter().any(|model| model.id == "gpt-5.6-sol"));
    let (init, _) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await
    })
    .await;
    let meta = init.meta.expect("provider discovery metadata");
    assert_eq!(meta["magistr"]["provider"], "codex");
    assert_eq!(meta["magistr"]["models"][0]["id"], "gpt-6-astra");
    assert_eq!(meta["magistr"]["capabilities"]["library_steering"], true);
    assert_eq!(
        meta["magistr"]["capabilities"]["terminal_cancellation_ack"],
        true
    );
    assert_eq!(meta["magistr"]["session_options"]["effort"], true);
    let inputs = recording.inputs();
    let methods: Vec<_> = inputs
        .iter()
        .filter_map(|frame| frame["method"].as_str())
        .collect();
    assert_eq!(
        methods,
        ["initialize", "initialized", "account/read", "model/list"]
    );
    assert!(
        inputs[0]["_argv"]
            .as_array()
            .expect("argv")
            .windows(3)
            .any(|args| { args == [json!("app-server"), json!("--listen"), json!("stdio://")] })
    );
    assert_eq!(
        recording.request("account/read")["params"]["refreshToken"],
        false
    );
}

#[tokio::test]
async fn a_thread_streams_once_reports_usage_and_exposes_its_persisted_record() {
    let recording = Recording::new("turn_message");
    let provider = recording.provider().await;
    let direct = provider.clone();
    let cwd = tempfile::tempdir().expect("session cwd");
    let (stop, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = cx
            .send_request(
                NewSessionRequest::new(cwd.path()).mcp_servers(vec![McpServer::Stdio(
                    McpServerStdio::new("fixture-tool", "/fixture/tool")
                        .args(vec!["serve".into()])
                        .env(vec![EnvVariable::new("FIXTURE_SETTING", "value")]),
                )]),
            )
            .block_task()
            .await?;
        assert_eq!(session.session_id.to_string(), THREAD);
        let picker = serde_json::to_value(session.config_options).expect("picker JSON");
        assert_eq!(picker[0]["currentValue"], "gpt-5.6-sol");
        let stop = prompt_and_wait(
            &cx,
            &session.session_id,
            "Reply with exactly the single word: pong",
        )
        .await?
        .stop_reason;
        let record = direct
            .0
            .read_thread(THREAD)
            .await
            .expect("recorded thread/read");
        assert_eq!(record.id, THREAD);
        assert!(
            record
                .replay
                .iter()
                .any(|u| matches!(u, SessionUpdate::UserMessageChunk(_)))
        );
        assert!(
            record
                .replay
                .iter()
                .any(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
        );
        assert_eq!(record.raw["id"], THREAD);
        Ok(stop)
    })
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(
        observed.messages(),
        "pong",
        "item/completed must not duplicate streamed text"
    );
    assert!(
        observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(usage) if usage.size == 258_400))
    );
    let start = recording.request("thread/start");
    assert_eq!(
        start["params"]["cwd"],
        cwd.path().to_string_lossy().as_ref()
    );
    assert_eq!(start["params"]["approvalPolicy"], "untrusted");
    assert_eq!(start["params"]["sandbox"], "workspace-write");
    assert_eq!(start["params"]["modelProvider"], "openai");
    let config = &start["params"]["config"];
    assert!(config.get("forced_login_method").is_none());
    assert_eq!(
        config["mcp_servers"]["fixture-tool"]["command"],
        "/fixture/tool"
    );
    assert_eq!(
        config["mcp_servers"]["fixture-tool"]["args"],
        json!(["serve"])
    );
    assert_eq!(
        config["mcp_servers"]["fixture-tool"]["env"]["FIXTURE_SETTING"],
        "value"
    );
    let turn = recording.request("turn/start");
    assert_eq!(turn["params"]["threadId"], THREAD);
    assert_eq!(turn["params"]["model"], "gpt-5.6-sol");
    assert_eq!(turn["params"]["effort"], "low");
    assert_eq!(
        turn["params"]["input"][0]["text"],
        "Reply with exactly the single word: pong"
    );
    assert_eq!(
        recording.request("thread/read")["params"]["includeTurns"],
        true
    );
}

#[tokio::test]
async fn a_model_switch_is_forwarded_on_the_next_turn() {
    let recording = Recording::new("turn_message");
    let (stop, _) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            cx.send_request(SetSessionConfigOptionRequest::new(
                session.session_id.clone(),
                "model",
                SessionConfigOptionValue::ValueId {
                    value: "gpt-6-astra".into(),
                },
            ))
            .block_task()
            .await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                .await?
                .stop_reason)
        },
    )
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(
        recording.request("turn/start")["params"]["model"],
        "gpt-6-astra"
    );
}

async fn approval_case(name: &str, option: &'static str, decision: &str, status: ToolCallStatus) {
    let recording = Recording::new(name);
    let (stop, observed) = with_provider(
        recording.provider().await,
        PermissionAnswer::Select(option),
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(
                prompt_and_wait(&cx, &session.session_id, "run the recorded command")
                    .await?
                    .stop_reason,
            )
        },
    )
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(
        *observed.permission_requests.lock().expect("permissions"),
        ["Bash"]
    );
    assert!(observed.saw(|u| match u {
        SessionUpdate::ToolCall(call) => call.raw_input.as_ref().is_some_and(|input| {
            input["command"]
                .as_str()
                .is_some_and(|command| command.contains("echo hello"))
        }),
        _ => false,
    }));
    assert!(observed.saw(|u| matches!(u, SessionUpdate::ToolCallUpdate(update) if update.fields.status == Some(status))));
    let response = recording
        .inputs()
        .into_iter()
        .find(|frame| frame.get("result").is_some())
        .expect("approval answer");
    assert_eq!(response, json!({"id": 0, "result": {"decision": decision}}));
    let expected = if status == ToolCallStatus::Completed {
        "done"
    } else {
        "failed"
    };
    assert!(observed.messages().ends_with(expected));
}

#[tokio::test]
async fn command_approval_accept_reaches_the_runtime_with_the_server_request_id() {
    approval_case(
        "command_approval_accept",
        "allow",
        "accept",
        ToolCallStatus::Completed,
    )
    .await;
}

#[tokio::test]
async fn command_approval_decline_marks_the_tool_failed_and_still_ends_the_turn() {
    approval_case(
        "command_approval_decline",
        "reject",
        "decline",
        ToolCallStatus::Failed,
    )
    .await;
}

#[tokio::test]
async fn runtime_death_ends_a_turn_while_the_client_holds_approval_open() {
    let mut recording = Recording::new("command_approval_accept");
    let approval = recording
        .frames
        .iter()
        .position(|frame| frame["method"] == "item/commandExecution/requestApproval")
        .expect("recorded command approval");
    recording.frames.truncate(approval + 1);
    recording
        .frames
        .extend([json!({"_delay_ms": 250}), json!({"_fault": "death"})]);
    let (error, observed) = with_provider(
        recording.provider().await,
        PermissionAnswer::Hold,
        async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let running = cx.send_request(prompt(&session.session_id, "run the recorded command"));
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    if observed
                        .pending_permission
                        .lock()
                        .expect("held permission")
                        .is_some()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("the ACP client received and held the approval");
            Ok(
                tokio::time::timeout(Duration::from_secs(1), running.block_task())
                    .await
                    .expect("runtime death must wake a provider waiting on client approval")
                    .expect_err("a dead runtime cannot complete a held turn"),
            )
        },
    )
    .await;
    assert!(
        observed
            .pending_permission
            .lock()
            .expect("held permission")
            .is_some(),
        "the prompt must fail without the client answering its permission request"
    );
    let text = serde_json::to_string(&error).expect("ACP error JSON");
    assert!(
        text.contains("Codex") && (text.contains("exited") || text.contains("disconnected")),
        "{text}"
    );
    assert!(
        recording
            .inputs()
            .iter()
            .all(|frame| frame.get("result").is_none()),
        "no approval response was sent before the runtime died"
    );
}

#[tokio::test]
async fn steering_checks_the_active_turn_and_interrupt_waits_for_terminal_notification() {
    let mut recording = Recording::new("turn_interrupt");
    let response = recording.response("turn/interrupt");
    recording
        .frames
        .insert(response + 1, json!({"_delay_ms": 180}));
    let provider = recording.provider().await;
    let direct = provider.clone();
    let ((stop, elapsed), observed) =
        with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let counting = cx.send_request(prompt(&session.session_id, "count to 400"));
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
                .await;
            direct
                .0
                .steer(
                    &session.session_id.to_string(),
                    INTERRUPT_TURN,
                    "Also, stop early.",
                )
                .await
                .expect("steer active turn");
            let error = direct
                .0
                .steer(
                    &session.session_id.to_string(),
                    "turn-that-is-not-active",
                    "never delivered",
                )
                .await
                .expect_err("wrong turn is refused");
            assert!(
                error.to_string().contains("expected active turn id"),
                "{error}"
            );
            let start = Instant::now();
            cx.send_notification(CancelNotification::new(session.session_id.clone()))?;
            let result = counting.block_task().await?;
            Ok((result.stop_reason, start.elapsed()))
        })
        .await;
    assert_eq!(stop, StopReason::Cancelled);
    assert!(
        elapsed >= Duration::from_millis(150),
        "interrupt acknowledgment must wait for turn/completed: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(1500),
        "the provider must finish before the session worker's two-second cancellation grace: {elapsed:?}"
    );
    assert_eq!(observed.messages(), "one\n");
    let interrupt = recording.request("turn/interrupt");
    assert_eq!(interrupt["params"]["turnId"], INTERRUPT_TURN);
    let steer = recording.request("turn/steer");
    assert_eq!(steer["params"]["expectedTurnId"], INTERRUPT_TURN);
    assert_eq!(steer["params"]["input"][0]["text"], "Also, stop early.");
}

fn interruption_recording() -> Recording {
    let mut recording = Recording::new("turn_interrupt");
    let steer = recording.marker("turn/steer");
    let interrupt = recording.marker("turn/interrupt");
    // Retain the recorded interrupt exchange while omitting the optional
    // steering exchanges preceding it, so cancellation is the only input.
    recording.frames.drain(steer..interrupt);
    recording
}

async fn cancel_recorded_turn(
    recording: &Recording,
) -> (
    Result<PromptResponse, agent_client_protocol::Error>,
    Duration,
) {
    let (result, _) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let running = cx.send_request(prompt(&session.session_id, "count to 400"));
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
                .await;
            let started = Instant::now();
            cx.send_notification(CancelNotification::new(session.session_id.clone()))?;
            Ok((running.block_task().await, started.elapsed()))
        },
    )
    .await;
    result
}

#[tokio::test]
async fn cancellation_waits_beyond_the_generic_worker_grace_for_codex_terminal_acknowledgment() {
    let mut recording = interruption_recording();
    let response = recording.response("turn/interrupt");
    recording.frames.splice(
        response + 1..response + 1,
        [
            json!({"_delay_ms": 1000}),
            json!({"_delay_ms": 1000}),
            json!({"_delay_ms": 300}),
        ],
    );
    let (result, elapsed) = cancel_recorded_turn(&recording).await;
    assert_eq!(
        result.expect("runtime confirmed interruption").stop_reason,
        StopReason::Cancelled
    );
    assert!(
        elapsed >= Duration::from_millis(2200),
        "ACP must wait for Codex terminal acknowledgment beyond the generic two-second grace: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "the recorded terminal must finish the prompt: {elapsed:?}"
    );
}

#[tokio::test]
async fn a_rejected_runtime_interrupt_is_an_acp_error_even_after_client_cancellation() {
    let mut recording = interruption_recording();
    let response = recording.response("turn/interrupt");
    let id = recording.frames[response]["id"].clone();
    recording.frames.truncate(response);
    recording.frames.push(json!({"id": id, "error": {
        "code": -32600, "message": "interrupt rejected: permission denied"
    }}));
    let (result, elapsed) = cancel_recorded_turn(&recording).await;
    let error = result.expect_err("a cancelled token cannot hide a rejected runtime interrupt");
    let text = serde_json::to_string(&error).expect("ACP error JSON");
    assert!(
        text.contains("interrupt rejected: permission denied"),
        "{text}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "the runtime refusal must reach ACP promptly: {elapsed:?}"
    );
}

#[tokio::test]
async fn a_terminal_queued_after_interrupt_refusal_allows_the_next_turn_without_reinterrupting() {
    let mut recording = interruption_recording();
    let response = recording.response("turn/interrupt");
    let id = recording.frames[response]["id"].clone();
    recording.frames[response] = json!({"id": id, "error": {
        "code": -32600, "message": "interrupt raced with turn completion"
    }});
    recording
        .frames
        .insert(response + 1, json!({"_delay_ms": 50}));
    let thread_id = recording.frames[recording.marker("turn/start")]["_sent"]["params"]["threadId"]
        .as_str()
        .expect("recorded runtime thread")
        .to_owned();
    // The read response follows the terminal on the same stdout stream.
    // Awaiting it proves that the routing owner has queued completion;
    // elapsed time cannot establish that under a loaded test runner.
    let mut snapshot =
        recording.frames[recording.response("thread/start")]["result"]["thread"].clone();
    snapshot["turns"] =
        json!([recording.frames.last().expect("terminal frame")["params"]["turn"].clone()]);
    // This injected read reply hydrates the fixture's empty terminal item list.
    snapshot["turns"][0]["itemsView"] = json!("full");
    recording.frames.extend([
        json!({"_sent": {"id": "after-interrupt", "method": "thread/read",
            "params": {"threadId": thread_id, "includeTurns": true}}}),
        json!({"id": "after-interrupt", "result": {"thread": snapshot}}),
    ]);
    let next = Recording::new("turn_message");
    let start = next.marker("turn/start");
    let end = next.marker("thread/read");
    // Compose a second recorded turn on the same runtime thread. The fake
    // still insists on exact request order: a redundant interrupt fails it.
    recording
        .frames
        .extend(next.frames[start..end].iter().map(|frame| {
            serde_json::from_str::<Value>(&frame.to_string().replace(THREAD, &thread_id))
                .expect("thread identity remapping preserves recorded JSON")
        }));
    let provider = recording.provider().await;
    let direct = provider.clone();
    let (stop, observed) =
        with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let running = cx.send_request(prompt(&session.session_id, "count to 400"));
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AgentMessageChunk(_)))
                .await;
            cx.send_notification(CancelNotification::new(session.session_id.clone()))?;
            running
                .block_task()
                .await
                .expect_err("interrupt refusal reaches ACP");
            direct
                .0
                .read_thread(&thread_id)
                .await
                .expect("read response follows the routed terminal");
            Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                .await?
                .stop_reason)
        })
        .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert!(observed.messages().ends_with("pong"));
    assert_eq!(
        recording
            .inputs()
            .iter()
            .filter(|frame| frame["method"] == "turn/interrupt")
            .count(),
        1
    );
}

#[tokio::test]
async fn resume_replays_history_restores_the_model_and_surfaces_missing_threads() {
    if !in_env::in_env(&[]) {
        return;
    }
    let recording = Recording::new("thread_resume");
    let provider = recording.provider().await;
    let direct = provider.clone();
    let (stop, observed) =
        with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
            initialize(&cx).await?;
            let sid = SessionId::new(THREAD);
            let loaded = cx
                .send_request(LoadSessionRequest::new(sid.clone(), std::env::temp_dir()))
                .block_task()
                .await?;
            assert!(observed.saw(|u| matches!(u, SessionUpdate::UserMessageChunk(_))));
            assert_eq!(observed.messages(), "pong");
            let picker = serde_json::to_value(loaded.config_options).expect("picker JSON");
            assert_eq!(picker[0]["currentValue"], "gpt-5.6-sol");
            let stop = prompt_and_wait(&cx, &sid, "which word did you say")
                .await?
                .stop_reason;
            let missing = direct
                .0
                .start(SessionStart {
                    session_id: SessionId::new("00000000-0000-0000-0000-000000000000"),
                    cwd: std::env::temp_dir(),
                    mcp_servers: Vec::new(),
                    resume: true,
                    model: None,
                    effort: None,
                    system_prompt: None,
                })
                .await
                .expect_err("missing persisted thread");
            assert!(
                missing.to_string().contains("no rollout found"),
                "{missing}"
            );
            Ok(stop)
        })
        .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(observed.messages(), "pongpong");
    let resume = recording.request("thread/resume");
    assert!(
        resume["params"].get("model").is_none(),
        "resume must preserve the persisted model"
    );
}

#[tokio::test]
async fn session_metadata_applies_model_and_effort_before_the_first_turn() {
    let mut recording = Recording::new("turn_message");
    let response = recording.response("thread/start");
    recording.frames[response]["result"]["model"] = json!("gpt-5.6-sol");
    let provider = recording.provider().await;
    with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let meta: agent_client_protocol::schema::v1::Meta = serde_json::from_value(
            json!({"magistr":{"session":{"model":"gpt-5.6-sol","effort":"high"}}}),
        )
        .expect("session metadata");
        let session = cx
            .send_request(NewSessionRequest::new(std::env::temp_dir()).meta(meta))
            .block_task()
            .await?;
        let picker = serde_json::to_value(session.config_options).expect("picker");
        assert_eq!(picker[0]["currentValue"], "gpt-5.6-sol");
        prompt_and_wait(&cx, &session.session_id, "pong").await
    })
    .await;
    assert_eq!(
        recording.request("thread/start")["params"]["model"],
        "gpt-5.6-sol"
    );
    assert_eq!(
        recording.request("thread/start")["params"]["config"]["model_reasoning_effort"],
        "high"
    );
    assert_eq!(
        recording.request("turn/start")["params"]["model"],
        "gpt-5.6-sol"
    );
    assert_eq!(recording.request("turn/start")["params"]["effort"], "high");
}

#[tokio::test]
async fn unsupported_session_effort_is_refused_before_starting_a_runtime_thread() {
    let recording = Recording::new("turn_message");
    let provider = recording.provider().await;
    with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let meta: agent_client_protocol::schema::v1::Meta =
            serde_json::from_value(json!({"magistr":{"session":{"effort":"impossible"}}}))
                .expect("session metadata");
        cx.send_request(NewSessionRequest::new(std::env::temp_dir()).meta(meta))
            .block_task()
            .await
            .expect_err("effort validation precedes thread start");
        Ok(())
    })
    .await;
    assert!(
        !recording
            .inputs()
            .iter()
            .any(|frame| frame["method"] == "thread/start")
    );
}

#[tokio::test]
async fn discovery_requires_a_chatgpt_account_and_nonempty_model_catalog() {
    for account in [Value::Null, json!({"type": "apiKey"})] {
        let mut recording = Recording::new("turn_message");
        let response = recording.response("account/read");
        recording.frames[response]["result"]["account"] = account;
        let error = recording
            .discover()
            .await
            .err()
            .expect("ChatGPT login is required");
        assert!(
            error.to_string().to_lowercase().contains("chatgpt"),
            "{error}"
        );
        assert!(
            recording
                .inputs()
                .iter()
                .all(|frame| frame["method"] != "thread/start")
        );
    }
    let mut recording = Recording::new("turn_message");
    let response = recording.response("model/list");
    recording.frames[response]["result"]["data"] = json!([]);
    let error = recording
        .discover()
        .await
        .err()
        .expect("empty catalog is unusable");
    assert!(
        error.to_string().to_lowercase().contains("model"),
        "{error}"
    );
}

#[tokio::test]
async fn a_runtime_authentication_change_refuses_new_work_without_api_fallback() {
    for auth_mode in [json!("apiKey"), Value::Null] {
        let mut recording = Recording::new("turn_message");
        let at = recording.marker("thread/start");
        // Inject a runtime account change after successful ChatGPT discovery.
        recording.frames.insert(
            at,
            json!({"method": "account/updated", "params": {
                "authMode": auth_mode, "planType": null
            }}),
        );
        let (error, _) = with_provider(
            recording.provider().await,
            PermissionAnswer::Cancel,
            async |cx, _| {
                initialize(&cx).await?;
                // The notification follows the final discovery response on a
                // separate pipe write; let the stdout reader receive it first.
                tokio::time::sleep(Duration::from_millis(40)).await;
                Ok(new_session(&cx)
                    .await
                    .expect_err("changed credentials must refuse new work"))
            },
        )
        .await;
        let text = serde_json::to_string(&error).expect("ACP error JSON");
        assert!(
            text.contains("ChatGPT") && text.contains("fallback is disabled"),
            "{text}"
        );
        assert!(
            recording.inputs().iter().all(|frame| {
                !matches!(
                    frame["method"].as_str(),
                    Some("thread/start" | "turn/start" | "account/login/start")
                )
            }),
            "authentication changes must fail before spawning model work or changing login"
        );
    }
}

#[tokio::test]
async fn a_missing_codex_binary_fails_with_its_path() {
    let error = CodexProvider::discover("/nonexistent/magistr-codex-test")
        .await
        .err()
        .expect("the binary does not exist");
    assert!(
        error
            .to_string()
            .contains("/nonexistent/magistr-codex-test"),
        "{error}"
    );
}

#[tokio::test]
async fn malformed_runtime_frames_fail_the_pending_turn_without_hanging() {
    let mut recording = Recording::new("turn_message");
    let at = recording.response("turn/start") + 1;
    recording.frames.insert(at, json!({"_fault": "malformed"}));
    let (error, _) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                .await
                .expect_err("malformed runtime output cannot complete a turn"))
        },
    )
    .await;
    let text = serde_json::to_string(&error).expect("ACP error JSON");
    assert!(
        text.contains("malformed") && text.contains("Codex"),
        "{text}"
    );
}

#[tokio::test]
async fn mcp_tool_permissions_reach_the_client_and_answer_with_elicitation_actions() {
    for (permission, action) in [
        (PermissionAnswer::Select("allow"), "accept"),
        (PermissionAnswer::Cancel, "cancel"),
    ] {
        let mut recording = Recording::new("turn_message");
        let at = recording.response("turn/start") + 1;
        let turn_id = recording.frames[at - 1]["result"]["turn"]["id"].clone();
        // Protocol-shaped injection; the live window check covers the real runtime.
        recording.frames.splice(at..at, [
            json!({"id": "source-permission", "method": "mcpServer/elicitation/request", "params": {
                "threadId": THREAD, "turnId": turn_id, "serverName": "workspace",
                "mode": "form", "message": "Allow file read?",
                "requestedSchema": {"type":"object","properties":{}},
                "_meta": {"codex_approval_kind":"mcp_tool_call","tool_params":{"file":"sample.txt"}}
            }}),
            json!({"_sent":{"id":"source-permission","result": if action == "accept" { json!({"action":action,"content":{}}) } else { json!({"action":action}) }}}),
        ]);
        let (stop, observed) =
            with_provider(recording.provider().await, permission, async |cx, _| {
                initialize(&cx).await?;
                let session = new_session(&cx).await?;
                Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                    .await?
                    .stop_reason)
            })
            .await;
        assert_eq!(stop, StopReason::EndTurn);
        assert_eq!(observed.messages(), "pong");
        let answer = recording
            .inputs()
            .into_iter()
            .find(|frame| frame["id"] == "source-permission")
            .unwrap();
        assert_eq!(answer["result"]["action"], action);
        assert!(answer["result"].get("decision").is_none());
    }
}

#[tokio::test]
async fn unsupported_server_requests_receive_a_correlated_error_and_the_turn_continues() {
    let mut recording = Recording::new("turn_message");
    let at = recording.response("turn/start") + 1;
    let turn_id = recording.frames[at - 1]["result"]["turn"]["id"].clone();
    // Explicit fault injection using the checked-in ToolRequestUserInputParams
    // schema. This capability is not served by the subscription provider.
    recording.frames.splice(at..at, [
        json!({"id": "unsupported-input", "method": "item/tool/requestUserInput", "params": {
            "threadId": THREAD, "turnId": turn_id, "itemId": "input-question",
            "isBlocking": true, "questions": [{"id": "direction", "header": "Direction", "question": "Continue?"}]
        }}),
        json!({"_sent": {"id": "unsupported-input", "error": {"code": -32601}}}),
    ]);
    let (stop, observed) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                .await?
                .stop_reason)
        },
    )
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(observed.messages(), "pong");
    let answer = recording
        .inputs()
        .into_iter()
        .find(|frame| frame["id"] == "unsupported-input")
        .expect("unsupported request answer");
    assert_eq!(answer["error"]["code"], -32601);
    assert!(
        answer["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("item/tool/requestUserInput"))
    );
}

#[tokio::test]
async fn a_terminal_notification_without_thread_identity_fails_the_pending_turn() {
    let mut recording = Recording::new("turn_message");
    let terminal = recording
        .frames
        .iter_mut()
        .find(|frame| frame["method"] == "turn/completed")
        .expect("recorded terminal");
    terminal["params"]
        .as_object_mut()
        .expect("notification params")
        .remove("threadId");
    let (error, _) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(tokio::time::timeout(
                Duration::from_secs(1),
                prompt_and_wait(&cx, &session.session_id, "pong"),
            )
            .await
            .expect("malformed terminal notification must fail every waiter promptly")
            .expect_err("missing thread identity cannot end a turn successfully"))
        },
    )
    .await;
    let text = serde_json::to_string(&error).expect("ACP error JSON");
    assert!(
        text.contains("threadId") && text.contains("malformed"),
        "{text}"
    );
}

#[tokio::test]
async fn eof_and_process_death_end_pending_turns_with_an_error() {
    for fault in ["eof", "death"] {
        let mut recording = Recording::new("turn_message");
        let at = recording.response("turn/start") + 1;
        recording.frames.truncate(at);
        recording.frames.push(json!({"_fault": fault}));
        let (error, _) = with_provider(
            recording.provider().await,
            PermissionAnswer::Cancel,
            async |cx, _| {
                initialize(&cx).await?;
                let session = new_session(&cx).await?;
                Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                    .await
                    .expect_err("a dead runtime cannot finish a turn"))
            },
        )
        .await;
        let text = serde_json::to_string(&error).expect("ACP error JSON");
        assert!(
            text.contains("Codex") || text.contains("codex"),
            "{fault}: {text}"
        );
    }
}

#[tokio::test]
async fn a_failed_terminal_turn_surfaces_its_codex_error_classification() {
    let mut recording = Recording::new("turn_message");
    let terminal = recording
        .frames
        .iter_mut()
        .find(|frame| frame["method"] == "turn/completed")
        .expect("recorded terminal notification");
    terminal["params"]["turn"]["status"] = json!("failed");
    terminal["params"]["turn"]["error"] = json!({
        "message": "This turn exceeded the context window",
        "codexErrorInfo": "contextWindowExceeded", "additionalDetails": null
    });
    let (error, _) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                .await
                .expect_err("failed terminal status cannot answer success"))
        },
    )
    .await;
    let text = serde_json::to_string(&error).expect("ACP error JSON");
    assert!(
        text.contains("contextWindowExceeded") && text.contains("context window"),
        "{text}"
    );
}

#[tokio::test]
async fn quota_failure_and_unavailable_model_are_actionable_errors() {
    // These JSON-RPC errors are explicit injected failure cases, shaped from
    // the checked-in App Server error schema, not live quota exhaustion.
    let mut recording = Recording::new("turn_message");
    let response = recording.response("turn/start");
    let id = recording.frames[response]["id"].clone();
    recording.frames.truncate(response);
    recording.frames.push(json!({"id": id, "error": {
        "code": -32000, "message": "ChatGPT usage limit reached; try again later",
        "data": {"codexErrorInfo": "usageLimitExceeded"}
    }}));
    let (error, _) = with_provider(
        recording.provider().await,
        PermissionAnswer::Cancel,
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "pong")
                .await
                .expect_err("quota failure"))
        },
    )
    .await;
    let text = serde_json::to_string(&error).expect("ACP error JSON");
    assert!(
        text.contains("usageLimitExceeded") && text.contains("try again later"),
        "{text}"
    );

    let mut recording = Recording::new("turn_message");
    let response = recording.response("thread/start");
    let id = recording.frames[response]["id"].clone();
    recording.frames.truncate(response);
    recording.frames.push(json!({"id": id, "error": {
        "code": -32600, "message": "The requested model is not available for this ChatGPT account"
    }}));
    let provider = recording.provider().await;
    let error = provider
        .start(SessionStart {
            session_id: SessionId::new("proposed"),
            cwd: std::env::temp_dir(),
            mcp_servers: Vec::new(),
            resume: false,
            model: None,
            effort: None,
            system_prompt: None,
        })
        .await
        .expect_err("unavailable runtime model");
    assert!(
        error
            .to_string()
            .contains("not available for this ChatGPT account"),
        "{error}"
    );
}
