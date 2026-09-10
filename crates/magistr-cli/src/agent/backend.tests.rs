use super::*;
use crate::agent::message::{AgentMessage, ContentBlock, UserBody};
use crate::agent::session::AcpSessionState;
use agent_client_protocol::schema::v1::{
    ContentBlock as AcpContentBlock, ContentChunk, Plan, PlanEntry, PlanEntryPriority,
    PlanEntryStatus, PromptResponse, SessionUpdate, StopReason, TextContent,
    ToolCall as AcpToolCall, ToolCallId, ToolCallStatus, ToolCallUpdate, ToolKind,
};

/// Uses the production ACP server while tracking whether startup inferred.
struct StartupProvider {
    prompts: Arc<AtomicU32>,
    has_models: bool,
}

impl magistr_acp_agent::Provider for StartupProvider {
    fn name(&self) -> &str {
        "startup-test"
    }

    fn models(&self) -> Vec<magistr_acp_agent::ModelChoice> {
        if self.has_models {
            magistr_acp_agent::ScriptedProvider.models()
        } else {
            Vec::new()
        }
    }

    fn start(
        &self,
        session: magistr_acp_agent::SessionStart,
    ) -> agent_client_protocol::BoxFuture<
        '_,
        std::result::Result<magistr_acp_agent::SessionOpened, magistr_acp_agent::AgentError>,
    > {
        magistr_acp_agent::ScriptedProvider.start(session)
    }

    fn prompt(
        &self,
        turn: magistr_acp_agent::Turn,
    ) -> agent_client_protocol::BoxFuture<
        '_,
        std::result::Result<StopReason, magistr_acp_agent::AgentError>,
    > {
        self.prompts.fetch_add(1, Ordering::SeqCst);
        magistr_acp_agent::ScriptedProvider.prompt(turn)
    }
}

async fn assert_model_selection_fails_before_prompt(has_models: bool) {
    let (client, agent) = agent_client_protocol::Channel::duplex();
    let prompts = Arc::new(AtomicU32::new(0));
    let server = tokio::spawn(magistr_acp_agent::serve(
        StartupProvider {
            prompts: Arc::clone(&prompts),
            has_models,
        },
        agent,
    ));
    let cwd = tempfile::tempdir().expect("session cwd");
    let mut transport = magistr_acp::AcpTransport::new_with_args(
        std::path::PathBuf::from("unused-in-process-agent"),
        Vec::new(),
        Some(cwd.path().to_path_buf()),
    )
    .expect("transport")
    .session_model(Some("unavailable-model".into()));
    let result = transport.connect_over(client).await;
    let error = match result {
        Err(error) => error,
        Ok(()) => {
            panic!("a requested model that cannot be selected must reject startup");
        }
    };
    assert!(error.to_string().contains("unavailable-model"), "{error}");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .expect("failed startup closes its ACP connection");
    assert_eq!(
        prompts.load(Ordering::SeqCst),
        0,
        "model-selection failure must not send a prompt"
    );
}

#[tokio::test]
async fn rejected_model_selection_fails_startup_before_inference() {
    assert_model_selection_fails_before_prompt(true).await;
}

#[tokio::test]
async fn missing_model_picker_fails_startup_before_inference() {
    assert_model_selection_fails_before_prompt(false).await;
}

#[tokio::test]
async fn model_startup_preserves_default_sentinel_and_honors_a_valid_override() {
    for (requested, expected) in [
        ("DeFaUlT", magistr_acp_agent::ScriptedProvider::FAST),
        (
            magistr_acp_agent::ScriptedProvider::SLOW,
            magistr_acp_agent::ScriptedProvider::SLOW,
        ),
    ] {
        let (client, agent) = agent_client_protocol::Channel::duplex();
        let prompts = Arc::new(AtomicU32::new(0));
        let server = tokio::spawn(magistr_acp_agent::serve(
            StartupProvider {
                prompts: Arc::clone(&prompts),
                has_models: true,
            },
            agent,
        ));
        let cwd = tempfile::tempdir().expect("session cwd");
        let mut transport = magistr_acp::AcpTransport::new_with_args(
            std::path::PathBuf::from("unused-in-process-agent"),
            Vec::new(),
            Some(cwd.path().to_path_buf()),
        )
        .expect("transport")
        .session_model((!requested.eq_ignore_ascii_case("default")).then(|| requested.into()));
        transport.connect_over(client).await.expect("ACP handshake");
        let (backend, mut messages, _) = AcpBackend::start_connected("which model", transport)
            .await
            .expect("selected model starts");
        let text = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            let mut text = String::new();
            loop {
                match messages
                    .recv()
                    .await
                    .expect("message channel")
                    .expect("agent message")
                {
                    AgentMessage::Assistant { message, .. } => {
                        for content in message.content {
                            if let ContentBlock::Text { text: chunk } = content {
                                text.push_str(&chunk);
                            }
                        }
                    }
                    AgentMessage::Result { is_error, .. } => {
                        assert!(!is_error);
                        return text;
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("scripted turn completes");
        assert_eq!(text, format!("[{expected}] echo: which model"));
        assert_eq!(prompts.load(Ordering::SeqCst), 1);
        drop(backend);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("backend drop closes the ACP connection");
    }
}

// ── convert_acp_event: SessionUpdate tests ──────────────────────────

#[test]
fn agent_message_chunk_converts_to_assistant() {
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::AgentMessageChunk(ContentChunk::new(
            AcpContentBlock::Text(TextContent::new("Hello, world!")),
        ))),
        session_id: "sess-123".to_string(),
    };

    let (msgs, is_result, is_cancelled) = convert_acp_event(&event);
    assert!(!is_result);
    assert!(!is_cancelled);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Assistant {
            message,
            session_id,
            ..
        } => {
            assert_eq!(message.content.len(), 1);
            assert!(matches!(
                &message.content[0],
                ContentBlock::Text { text } if text == "Hello, world!"
            ));
            assert_eq!(session_id.as_ref().unwrap().as_str(), "sess-123");
        }
        _ => panic!("Expected Assistant message"),
    }
}

#[test]
fn tool_call_converts_to_assistant_tool_use() {
    let tc = AcpToolCall::new(ToolCallId::new("tool-1"), "Read file")
        .kind(ToolKind::Read)
        .raw_input(serde_json::json!({"file_path": "/tmp/test.rs"}));
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::ToolCall(tc)),
        session_id: "sess-123".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Assistant { message, .. } => {
            assert_eq!(message.content.len(), 1);
            assert!(matches!(
                &message.content[0],
                ContentBlock::ToolUse { name, .. } if name == "Read file"
            ));
        }
        _ => panic!("Expected Assistant message"),
    }
}

#[test]
fn thought_chunk_converts_to_thinking_block() {
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::AgentThoughtChunk(ContentChunk::new(
            AcpContentBlock::Text(TextContent::new("Let me analyze this...")),
        ))),
        session_id: "sess-123".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Assistant { message, .. } => {
            assert!(matches!(
                &message.content[0],
                ContentBlock::Thinking { thinking, .. }
                    if thinking == "Let me analyze this..."
            ));
        }
        _ => panic!("Expected Assistant message"),
    }
}

#[test]
fn user_message_chunk_converts_to_user() {
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::UserMessageChunk(ContentChunk::new(
            AcpContentBlock::Text(TextContent::new("Fix the bug")),
        ))),
        session_id: "sess-456".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::User {
            message,
            session_id,
            ..
        } => {
            assert!(matches!(
                &message.content,
                Some(UserBody::String(s)) if s == "Fix the bug"
            ));
            assert_eq!(session_id.as_ref().unwrap().as_str(), "sess-456");
        }
        _ => panic!("Expected User message"),
    }
}

#[test]
fn plan_converts_to_plan_message() {
    let plan = Plan::new(vec![PlanEntry::new(
        "Step 1",
        PlanEntryPriority::High,
        PlanEntryStatus::Completed,
    )]);
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::Plan(plan)),
        session_id: "sess-plan".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Plan { entries } => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].content, "Step 1");
        }
        _ => panic!("Expected Plan message"),
    }
}

// ── convert_acp_event: ToolCallUpdate tests ─────────────────────────

#[test]
fn tool_call_update_completed_with_content_emits_tool_result() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-42"), "Read file.rs")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::Completed)
        .content(vec![
            AcpContentBlock::Text(TextContent::new("file contents here")).into(),
        ]);
    let tcu = ToolCallUpdate::from(tc);
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::ToolCallUpdate(tcu)),
        session_id: "sess-1".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    // Should produce StreamEvent + tool_result User message
    assert_eq!(msgs.len(), 2);
    assert!(matches!(&msgs[0], AgentMessage::StreamEvent { .. }));
    match &msgs[1] {
        AgentMessage::User { message, .. } => match &message.content {
            Some(UserBody::Blocks(blocks)) => match &blocks[0] {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    assert_eq!(tool_use_id, "tc-42");
                    assert_eq!(content.as_deref(), Some("file contents here"));
                    assert_eq!(*is_error, Some(false));
                }
                _ => panic!("Expected ToolResult content block"),
            },
            _ => panic!("Expected Blocks content"),
        },
        _ => panic!("Expected User message"),
    }
}

#[test]
fn tool_call_update_failed_marks_is_error() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-99"), "Bash")
        .kind(ToolKind::Execute)
        .status(ToolCallStatus::Failed)
        .raw_output(serde_json::Value::String("permission denied".to_string()));
    let tcu = ToolCallUpdate::from(tc);
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::ToolCallUpdate(tcu)),
        session_id: "sess-2".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 2);
    match &msgs[1] {
        AgentMessage::User { message, .. } => match &message.content {
            Some(UserBody::Blocks(blocks)) => match &blocks[0] {
                ContentBlock::ToolResult { is_error, .. } => {
                    assert_eq!(*is_error, Some(true));
                }
                _ => panic!("Expected ToolResult"),
            },
            _ => panic!("Expected Blocks content"),
        },
        _ => panic!("Expected User message"),
    }
}

#[test]
fn tool_call_update_in_progress_becomes_stream_event() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-10"), "Write file")
        .kind(ToolKind::Edit)
        .status(ToolCallStatus::InProgress);
    let tcu = ToolCallUpdate::from(tc);
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::ToolCallUpdate(tcu)),
        session_id: "sess-3".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 1);
    assert!(
        matches!(&msgs[0], AgentMessage::StreamEvent { uuid, .. } if uuid == "tc-10"),
        "In-progress update should become StreamEvent"
    );
}

#[test]
fn tool_call_update_completed_without_content_no_tool_result() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-11"), "Think")
        .kind(ToolKind::Think)
        .status(ToolCallStatus::Completed);
    let tcu = ToolCallUpdate::from(tc);
    let event = AcpEvent::SessionUpdate {
        update: Box::new(SessionUpdate::ToolCallUpdate(tcu)),
        session_id: "sess-4".to_string(),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    // No content → only the StreamEvent, no tool_result
    assert_eq!(msgs.len(), 1);
    assert!(matches!(&msgs[0], AgentMessage::StreamEvent { .. }));
}

// ── convert_acp_event: PromptResponse tests ─────────────────────────

#[test]
fn prompt_response_end_turn_is_result() {
    let event = AcpEvent::PromptResponse {
        response: PromptResponse::new(StopReason::EndTurn),
        session_id: "sess-789".to_string(),
    };

    let (msgs, is_result, is_cancelled) = convert_acp_event(&event);
    assert!(is_result);
    assert!(!is_cancelled);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Result {
            subtype,
            is_error,
            num_turns,
            ..
        } => {
            assert_eq!(subtype, "success");
            assert!(!is_error);
            assert_eq!(*num_turns, 1, "each ACP PromptResponse counts as 1 turn");
        }
        _ => panic!("Expected Result message"),
    }
}

#[test]
fn prompt_response_cancelled_is_cancelled() {
    let event = AcpEvent::PromptResponse {
        response: PromptResponse::new(StopReason::Cancelled),
        session_id: "sess-cancel".to_string(),
    };

    let (msgs, is_result, is_cancelled) = convert_acp_event(&event);
    assert!(!is_result);
    assert!(is_cancelled);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Cancelled { session_id, reason } => {
            assert_eq!(session_id.as_str(), "sess-cancel");
            assert!(reason.contains("cancelled"));
        }
        _ => panic!("Expected Cancelled message"),
    }
}

#[test]
fn prompt_response_max_tokens_is_error_result() {
    let event = AcpEvent::PromptResponse {
        response: PromptResponse::new(StopReason::MaxTokens),
        session_id: "sess-err".to_string(),
    };

    let (msgs, is_result, _) = convert_acp_event(&event);
    assert!(is_result);
    match &msgs[0] {
        AgentMessage::Result {
            subtype, is_error, ..
        } => {
            assert_eq!(subtype, "max_tokens");
            assert!(is_error);
        }
        _ => panic!("Expected Result message"),
    }
}

// ── convert_acp_event: System event tests ───────────────────────────

#[test]
fn system_event_converts_to_system_message() {
    let event = AcpEvent::System {
        subtype: "permission_request".to_string(),
        data: serde_json::json!({"tool_name": "Bash"}),
    };

    let (msgs, _, _) = convert_acp_event(&event);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::System { subtype, .. } => {
            assert_eq!(subtype, "permission_request");
        }
        _ => panic!("Expected System message"),
    }
}

// ── convert_acp_event: Initialized tests ──────────────────────────

#[test]
fn initialized_event_converts_to_system_with_capabilities() {
    use agent_client_protocol::schema::v1::{
        AgentCapabilities, PromptCapabilities, SessionCapabilities,
    };

    let caps = AgentCapabilities::new()
        .load_session(true)
        .prompt_capabilities(PromptCapabilities::new().image(true))
        .session_capabilities(SessionCapabilities::new());

    let event = AcpEvent::Initialized {
        agent_capabilities: Box::new(caps),
        session_id: "sess-init".to_string(),
        config_options: None,
        agent_meta: None,
    };

    let (msgs, is_result, is_cancelled) = convert_acp_event(&event);
    assert!(!is_result);
    assert!(!is_cancelled);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::System { subtype, data } => {
            assert_eq!(subtype, "initialized");
            assert_eq!(data["session_id"], "sess-init");
            assert_eq!(data["load_session"], true);
            assert_eq!(data["prompt_capabilities"]["image"], true);
            assert_eq!(data["prompt_capabilities"]["audio"], false);
            assert_eq!(data["session_list"], false);
        }
        _ => panic!("Expected System message"),
    }
}

#[test]
fn initialized_event_with_session_list_capability() {
    use agent_client_protocol::schema::v1::{
        AgentCapabilities, SessionCapabilities, SessionListCapabilities,
    };

    let caps = AgentCapabilities::new()
        .session_capabilities(SessionCapabilities::new().list(SessionListCapabilities::new()));

    let event = AcpEvent::Initialized {
        agent_capabilities: Box::new(caps),
        session_id: "sess-list".to_string(),
        config_options: None,
        agent_meta: None,
    };

    let (msgs, _, _) = convert_acp_event(&event);
    match &msgs[0] {
        AgentMessage::System { data, .. } => {
            assert_eq!(data["session_list"], true);
        }
        _ => panic!("Expected System message"),
    }
}

#[test]
fn initialized_event_with_config_options_emits_config_update() {
    use crate::agent::message::ConfigOptionCategory;
    use agent_client_protocol::schema::v1::{
        AgentCapabilities, SessionConfigOption, SessionConfigOptionCategory,
        SessionConfigSelectOption,
    };

    let opts = vec![
        SessionConfigOption::select(
            "model-id",
            "Model",
            "opus",
            vec![SessionConfigSelectOption::new("opus", "Opus")],
        )
        .category(SessionConfigOptionCategory::Model)
        .description("Select model"),
    ];

    let caps = AgentCapabilities::new();
    let event = AcpEvent::Initialized {
        agent_capabilities: Box::new(caps),
        session_id: "sess-cfg".to_string(),
        config_options: Some(opts),
        agent_meta: None,
    };

    let (msgs, _, _) = convert_acp_event(&event);
    // Should produce System + ConfigUpdate
    assert_eq!(msgs.len(), 2);
    assert!(matches!(&msgs[0], AgentMessage::System { subtype, .. } if subtype == "initialized"));
    match &msgs[1] {
        AgentMessage::ConfigUpdate { options } => {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].id, "model-id");
            assert_eq!(options[0].name, "Model");
            assert_eq!(options[0].category, Some(ConfigOptionCategory::Model));
            assert_eq!(options[0].current_value, "opus");
        }
        _ => panic!("Expected ConfigUpdate message"),
    }
}

#[test]
fn initialized_event_with_empty_config_options_no_config_update() {
    use agent_client_protocol::schema::v1::AgentCapabilities;

    let caps = AgentCapabilities::new();
    let event = AcpEvent::Initialized {
        agent_capabilities: Box::new(caps),
        session_id: "sess-empty".to_string(),
        config_options: Some(vec![]),
        agent_meta: None,
    };

    let (msgs, _, _) = convert_acp_event(&event);
    // Empty config options should not emit ConfigUpdate
    assert_eq!(msgs.len(), 1);
    assert!(matches!(&msgs[0], AgentMessage::System { .. }));
}

// ── AcpSessionState tests ───────────────────────────────────────────

#[test]
fn acp_session_state_debug_display() {
    let states = [
        AcpSessionState::Created,
        AcpSessionState::InProgress,
        AcpSessionState::Awaiting,
        AcpSessionState::Completed,
        AcpSessionState::Failed,
        AcpSessionState::Cancelled,
    ];
    for state in &states {
        let debug = format!("{state:?}");
        assert!(!debug.is_empty());
    }
}

#[test]
fn acp_session_state_copy() {
    let state = AcpSessionState::InProgress;
    let copy = state;
    assert_eq!(state, copy);
}

// ── State transition tests ──────────────────────────────────────────

#[tokio::test]
async fn state_transitions_created_to_completed() {
    let state = Arc::new(Mutex::new(AcpSessionState::Created));

    *state.lock().unwrap() = AcpSessionState::InProgress;
    assert_eq!(*state.lock().unwrap(), AcpSessionState::InProgress);

    *state.lock().unwrap() = AcpSessionState::Awaiting;
    assert_eq!(*state.lock().unwrap(), AcpSessionState::Awaiting);

    *state.lock().unwrap() = AcpSessionState::InProgress;
    assert_eq!(*state.lock().unwrap(), AcpSessionState::InProgress);

    *state.lock().unwrap() = AcpSessionState::Completed;
    assert_eq!(*state.lock().unwrap(), AcpSessionState::Completed);
}

#[tokio::test]
async fn state_transitions_cancel() {
    let state = Arc::new(Mutex::new(AcpSessionState::InProgress));
    *state.lock().unwrap() = AcpSessionState::Cancelled;
    assert_eq!(*state.lock().unwrap(), AcpSessionState::Cancelled);
}

#[tokio::test]
async fn state_transitions_error() {
    let state = Arc::new(Mutex::new(AcpSessionState::InProgress));
    *state.lock().unwrap() = AcpSessionState::Failed;
    assert_eq!(*state.lock().unwrap(), AcpSessionState::Failed);
}

// ── Turn counting ───────────────────────────────────────────────────

#[test]
fn turn_count_atomic_operations() {
    let count = Arc::new(AtomicU32::new(0));
    assert_eq!(count.load(Ordering::SeqCst), 0);

    count.fetch_add(1, Ordering::SeqCst);
    assert_eq!(count.load(Ordering::SeqCst), 1);

    count.fetch_add(1, Ordering::SeqCst);
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn terminal_bytes_convert_to_a_terminal_output_system_message() {
    let event = AcpEvent::Terminal {
        terminal_id: "term-1".to_string(),
        bytes: b"\x1b[31mred\x1b[0m\r\n".to_vec(),
    };
    let (msgs, is_result, is_cancelled) = convert_acp_event(&event);
    assert!(!is_result && !is_cancelled);
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::System { subtype, data } => {
            assert_eq!(subtype, "terminal_output");
            assert_eq!(data["terminal_id"], "term-1");
            let bytes: Vec<u8> = serde_json::from_value(data["bytes"].clone()).expect("bytes");
            assert_eq!(bytes, b"\x1b[31mred\x1b[0m\r\n");
        }
        _ => panic!("Expected System message"),
    }
}
