//! The Claude provider end to end — through the ACP agent and the scripted
//! client — against a fake `claude` (src/bin/fake_claude.rs) that replays the
//! frames recorded from the real one. Hermetic: no model, no login.

mod fake_client;
#[path = "../../magistr-cli/tests/support/in_env.rs"]
mod in_env;

use std::path::PathBuf;

use agent_client_protocol::schema::v1::{
    CancelNotification, LoadSessionRequest, SessionConfigOptionValue, SessionId, SessionUpdate,
    SetSessionConfigOptionRequest, StopReason, ToolCallStatus,
};
use fake_client::{
    PermissionAnswer, initialize, new_session, prompt, prompt_and_wait, with_provider,
};
use magistr_acp_agent::{ClaudeProvider, Provider};

fn fixture(name: &str) -> String {
    format!(
        "{}/tests/fixtures/claude/{name}.ndjson",
        env!("CARGO_MANIFEST_DIR")
    )
}

async fn provider(fixture_name: &str) -> ClaudeProvider {
    ClaudeProvider::discover_with(
        PathBuf::from(env!("CARGO_BIN_EXE_fake_claude")),
        vec![fixture(fixture_name)],
    )
    .await
    .expect("the fake claude answers the handshake")
}

fn json(update: &SessionUpdate) -> String {
    serde_json::to_string(update).expect("json")
}

#[tokio::test]
async fn discovery_reads_the_catalog_and_the_agent_advertises_it() {
    let provider = provider("init").await;
    assert_eq!(provider.name(), "claude");
    let ids: Vec<String> = provider.models().into_iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        [
            "default",
            "opus[1m]",
            "claude-fable-5-1[1m]",
            "sonnet",
            "haiku"
        ]
    );

    let (init, _) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await
    })
    .await;
    let meta = init.meta.expect("_meta.magistr");
    assert_eq!(meta["magistr"]["provider"], "claude");
    assert_eq!(meta["magistr"]["models"][2]["label"], "Fable");
}

#[tokio::test]
async fn a_new_session_gets_the_picker_and_the_command_list_and_two_turns_run() {
    let provider = provider("turn_thinking").await;
    let (stops, observed) =
        with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            assert!(
                uuid::Uuid::parse_str(&session.session_id.to_string()).is_ok(),
                "the session id is the CLI's --session-id: {}",
                session.session_id
            );
            let picker = session.config_options.expect("the model picker");
            assert_eq!(json_of(&picker), "default", "the CLI's default is current");
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AvailableCommandsUpdate(_)))
                .await;
            let first = prompt_and_wait(&cx, &session.session_id, "why is the sky blue").await?;
            let after_first = observed.messages();
            let second = prompt_and_wait(&cx, &session.session_id, "ping").await?;
            Ok((first.stop_reason, after_first, second.stop_reason))
        })
        .await;
    assert_eq!(stops.0, StopReason::EndTurn);
    assert_eq!(stops.1, "pong");
    assert_eq!(stops.2, StopReason::EndTurn);
    assert_eq!(observed.messages(), "pongping");
    assert!(
        observed.saw(|u| matches!(u, SessionUpdate::UsageUpdate(usage) if usage.size == 1_000_000))
    );
    let commands = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .find_map(|(_, u)| match u {
            SessionUpdate::AvailableCommandsUpdate(c) => Some(c.available_commands.clone()),
            _ => None,
        })
        .expect("the command list");
    assert!(commands.iter().any(|c| c.name == "goal"), "{commands:?}");
}

fn json_of(picker: &[agent_client_protocol::schema::v1::SessionConfigOption]) -> String {
    let value = serde_json::to_value(picker).expect("json");
    value[0]["currentValue"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn a_permission_ask_reaches_the_client_and_allow_completes_the_call() {
    let provider = provider("tool_permission_allow").await;
    let (stop, observed) = with_provider(
        provider,
        PermissionAnswer::Select("allow"),
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "write allow.txt")
                .await?
                .stop_reason)
        },
    )
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(
        *observed.permission_requests.lock().expect("lock"),
        vec!["Write".to_owned()],
        "the ask carries the raw tool name the client's policy keys on"
    );
    assert!(observed.saw(|u| matches!(u, SessionUpdate::ToolCall(call) if call.title == "Write")));
    assert!(observed.saw(|u| matches!(
        u,
        SessionUpdate::ToolCallUpdate(update) if update.fields.status == Some(ToolCallStatus::Completed)
    )));
    assert_eq!(observed.messages(), "written");
    assert!(observed.saw(|u| matches!(u, SessionUpdate::AgentThoughtChunk(_))));

    // The whole call crosses the wire: the tool_use input as rawInput
    // with the file it names as a location, the tool_result content as
    // a text block and whole as rawOutput.
    let (announced, completed) = {
        let updates = observed.updates.lock().expect("lock");
        let announced = updates
            .iter()
            .find_map(|(_, u)| matches!(u, SessionUpdate::ToolCall(_)).then(|| json(u)))
            .expect("the announcement");
        let completed = updates
            .iter()
            .find_map(|(_, u)| match u {
                SessionUpdate::ToolCallUpdate(update)
                    if update.fields.status == Some(ToolCallStatus::Completed) =>
                {
                    Some(json(u))
                }
                _ => None,
            })
            .expect("the completion");
        (announced, completed)
    };
    assert!(
        announced.contains(r#""rawInput":{"#) && announced.contains(r#""content":"hello""#),
        "{announced}"
    );
    assert!(
        announced.contains(r#""locations":[{"path":"#) && announced.contains("allow.txt"),
        "{announced}"
    );
    assert!(
        completed.contains(r#""content":[{"type":"content""#)
            && completed.contains(r#""rawOutput":"File created successfully"#),
        "{completed}"
    );
}

#[tokio::test]
async fn allow_always_is_relayed_too() {
    let provider = provider("tool_permission_always").await;
    let (stop, observed) = with_provider(
        provider,
        PermissionAnswer::Select("allow_always"),
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(
                prompt_and_wait(&cx, &session.session_id, "write always.txt")
                    .await?
                    .stop_reason,
            )
        },
    )
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(observed.messages(), "written");
}

#[tokio::test]
async fn a_rejected_ask_fails_the_call_and_the_turn_still_ends() {
    let provider = provider("tool_permission_deny").await;
    let (stop, observed) = with_provider(
        provider,
        PermissionAnswer::Select("reject"),
        async |cx, _| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            Ok(prompt_and_wait(&cx, &session.session_id, "write deny.txt")
                .await?
                .stop_reason)
        },
    )
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    let failed = observed
        .updates
        .lock()
        .expect("lock")
        .iter()
        .find_map(|(_, u)| match u {
            SessionUpdate::ToolCallUpdate(update)
                if update.fields.status == Some(ToolCallStatus::Failed) =>
            {
                Some(json(u))
            }
            _ => None,
        })
        .expect("the failed call");
    assert!(failed.contains("the user declined"), "{failed}");
    assert_eq!(observed.messages(), "refused");
}

#[tokio::test]
async fn cancel_mid_turn_interrupts_the_cli_and_ends_the_turn_cancelled() {
    let provider = provider("turn_cancelled").await;
    let (stop, observed) =
        with_provider(provider, PermissionAnswer::Cancel, async |cx, observed| {
            initialize(&cx).await?;
            let session = new_session(&cx).await?;
            let sid = session.session_id.clone();
            let counting = cx.send_request(prompt(&sid, "count to 400"));
            observed
                .wait_for(|u| matches!(u, SessionUpdate::AgentThoughtChunk(_)))
                .await;
            cx.send_notification(CancelNotification::new(sid.clone()))?;
            Ok(counting.block_task().await?.stop_reason)
        })
        .await;
    assert_eq!(stop, StopReason::Cancelled);
    assert_eq!(
        observed.messages(),
        "",
        "the CLI's interrupt note is not shown"
    );
}

#[tokio::test]
async fn session_load_resumes_and_replays_the_transcript() {
    let sid = SessionId::new("00000000-0000-4000-8000-0000000001e3");
    let cwd = tempfile::tempdir().expect("tempdir");
    let config = tempfile::tempdir().expect("tempdir");
    let encoded: String = cwd
        .path()
        .to_string_lossy()
        .chars()
        .map(|c| if c == '/' { '-' } else { c })
        .collect();
    let transcript_dir = config.path().join("projects").join(encoded);
    std::fs::create_dir_all(&transcript_dir).expect("mkdir");
    std::fs::copy(
        format!(
            "{}/tests/fixtures/claude/transcript.jsonl",
            env!("CARGO_MANIFEST_DIR")
        ),
        transcript_dir.join(format!("{sid}.jsonl")),
    )
    .expect("copy");

    let provider = provider("session_resume").await.config_dir(config.path());
    let (stop, observed) = with_provider(provider, PermissionAnswer::Cancel, {
        let sid = sid.clone();
        let cwd = cwd.path().to_path_buf();
        async move |cx, observed| {
            initialize(&cx).await?;
            cx.send_request(LoadSessionRequest::new(sid.clone(), cwd))
                .block_task()
                .await?;
            assert!(
                observed.saw(|u| matches!(u, SessionUpdate::UserMessageChunk(_))),
                "the transcript's prompts replayed before the load answered"
            );
            assert_eq!(
                observed.messages(),
                "pongpingpong",
                "the transcript's answers"
            );
            Ok(prompt_and_wait(&cx, &sid, "which word").await?.stop_reason)
        }
    })
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert!(observed.messages().ends_with("pong"));
    assert!(observed.saw(|u| matches!(u, SessionUpdate::AvailableCommandsUpdate(_))));
}

#[tokio::test]
async fn a_fresh_client_process_replays_claude_history_written_by_its_parent() {
    const CONFIG: &str = "MAGISTR_TEST_CLAUDE_HISTORY";
    const SESSION: &str = "00000000-0000-4000-8000-0000000001e3";
    let cwd = std::env::temp_dir();
    let config = match std::env::var_os(CONFIG) {
        Some(path) => PathBuf::from(path),
        None => {
            let config = tempfile::tempdir().expect("persisted Claude config");
            let encoded = cwd.to_string_lossy().replace('/', "-");
            let dir = config.path().join("projects").join(encoded);
            std::fs::create_dir_all(&dir).expect("projects directory");
            std::fs::write(
                dir.join(format!("{SESSION}.jsonl")),
                include_str!("fixtures/claude/transcript.jsonl"),
            )
            .expect("persist transcript before child exists");
            assert!(!in_env::in_env(&[(
                CONFIG,
                Some(config.path().as_os_str())
            )]));
            return;
        }
    };
    let provider = provider("session_resume").await.config_dir(config);
    let (_, observed) = with_provider(
        provider,
        PermissionAnswer::Cancel,
        async move |cx, observed| {
            let init = initialize(&cx).await?;
            assert!(init.agent_capabilities.load_session);
            let loaded = cx
                .send_request(LoadSessionRequest::new(SessionId::new(SESSION), cwd))
                .block_task()
                .await?;
            assert_eq!(
                observed.messages(),
                "pongpingpong",
                "disk history arrived before load response"
            );
            assert!(observed.saw(|update| matches!(update, SessionUpdate::AgentThoughtChunk(_))));
            assert!(
                loaded.config_options.is_none(),
                "unknown restored model must not become the current default"
            );
            Ok(())
        },
    )
    .await;
    assert_eq!(observed.messages(), "pongpingpong");
}

#[tokio::test]
async fn a_model_switch_is_sent_to_the_cli_before_the_next_turn() {
    let provider = provider("set_model").await;
    let (stop, observed) = with_provider(provider, PermissionAnswer::Cancel, async |cx, _| {
        initialize(&cx).await?;
        let session = new_session(&cx).await?;
        let sid = session.session_id.clone();
        cx.send_request(SetSessionConfigOptionRequest::new(
            sid.clone(),
            "model",
            SessionConfigOptionValue::ValueId {
                value: "sonnet".into(),
            },
        ))
        .block_task()
        .await?;
        Ok(prompt_and_wait(&cx, &sid, "pong please").await?.stop_reason)
    })
    .await;
    assert_eq!(stop, StopReason::EndTurn);
    assert_eq!(observed.messages(), "pong");
}

#[tokio::test]
async fn a_missing_binary_fails_discovery_with_the_path() {
    let Err(err) = ClaudeProvider::discover("/nonexistent/claude-binary").await else {
        panic!("no such binary");
    };
    assert!(
        err.to_string().contains("/nonexistent/claude-binary"),
        "{err}"
    );
}
