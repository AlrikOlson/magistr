use super::*;

#[test]
fn the_first_tool_observation_preserves_an_already_finished_result() {
    let call = AcpToolCall::new(ToolCallId::new("finished"), "Read file")
        .status(ToolCallStatus::Failed)
        .raw_output(serde_json::json!("Permission denied"));
    let messages = messages_from_session_update(&SessionUpdate::ToolCall(call), &test_session_id());
    assert!(matches!(&messages[0], AgentMessage::Assistant { .. }));
    let AgentMessage::StreamEvent { event, .. } = &messages[1] else {
        panic!("status and output")
    };
    assert_eq!(event["status"], "Failed");
    assert_eq!(event["raw_output"], "Permission denied");
}

#[test]
fn tool_output_is_available_before_completion_without_a_duplicate_result() {
    let call = AcpToolCall::new(ToolCallId::new("stream-output"), "cargo check")
        .status(ToolCallStatus::InProgress)
        .content(vec![
            AcpContentBlock::Text(TextContent::new("Checking workspace")).into(),
        ]);
    let update = SessionUpdate::ToolCallUpdate(ToolCallUpdate::from(call));
    let messages = messages_from_session_update(&update, &test_session_id());
    assert_eq!(messages.len(), 1);
    let AgentMessage::StreamEvent { event, .. } = &messages[0] else {
        panic!("stream event")
    };
    assert_eq!(event["content"], "Checking workspace");
}

use agent_client_protocol::schema::v1::{
    Content, ContentChunk, PlanEntry, PlanEntryPriority, PlanEntryStatus, TextContent, ToolCallId,
};

fn test_session_id() -> SessionId {
    SessionId::new("test-session-1")
}

// ── ContentBlock conversion ─────────────────────────────────────────

#[test]
fn acp_text_content_converts_to_text_block() {
    let acp = AcpContentBlock::Text(TextContent::new("hello world"));
    let block = content_block_from_acp(&acp);
    assert!(matches!(block, ContentBlock::Text { text } if text == "hello world"));
}

#[test]
fn acp_image_content_converts_to_image_block() {
    let acp = AcpContentBlock::Image(agent_client_protocol::schema::v1::ImageContent::new(
        "base64data",
        "image/png",
    ));
    let block = content_block_from_acp(&acp);
    match block {
        ContentBlock::Image {
            mime_type,
            uri,
            alt,
        } => {
            assert_eq!(mime_type, "image/png");
            assert!(uri.is_none());
            assert_eq!(alt, "image/png"); // falls back to mime_type when no URI
        }
        _ => panic!("expected Image, got {block:?}"),
    }
}

#[test]
fn acp_image_content_with_uri_extracts_filename() {
    let img = agent_client_protocol::schema::v1::ImageContent::new("base64data", "image/jpeg")
        .uri("https://example.com/photo.jpg");
    let acp = AcpContentBlock::Image(img);
    let block = content_block_from_acp(&acp);
    match block {
        ContentBlock::Image {
            mime_type,
            uri,
            alt,
        } => {
            assert_eq!(mime_type, "image/jpeg");
            assert_eq!(uri.as_deref(), Some("https://example.com/photo.jpg"));
            assert_eq!(alt, "photo.jpg"); // filename extracted from URI
        }
        _ => panic!("expected Image, got {block:?}"),
    }
}

#[test]
fn acp_audio_content_shows_mime_type() {
    let acp = AcpContentBlock::Audio(agent_client_protocol::schema::v1::AudioContent::new(
        "base64data",
        "audio/wav",
    ));
    let block = content_block_from_acp(&acp);
    assert!(matches!(block, ContentBlock::Text { text } if text.contains("audio/wav")));
}

#[test]
fn acp_resource_text_shows_uri_and_size() {
    let text_res = agent_client_protocol::schema::v1::TextResourceContents::new(
        "hello world",
        "file:///tmp/test.txt",
    );
    let res = agent_client_protocol::schema::v1::EmbeddedResource::new(
        agent_client_protocol::schema::v1::EmbeddedResourceResource::TextResourceContents(text_res),
    );
    let acp = AcpContentBlock::Resource(res);
    let block = content_block_from_acp(&acp);
    match block {
        ContentBlock::Text { text } => {
            assert!(text.contains("file:///tmp/test.txt"), "missing uri: {text}");
            assert!(text.contains("11 bytes"), "missing size: {text}");
        }
        _ => panic!("expected Text"),
    }
}

#[test]
fn acp_resource_link_shows_name_and_uri() {
    let link =
        agent_client_protocol::schema::v1::ResourceLink::new("README", "file:///project/README.md");
    let acp = AcpContentBlock::ResourceLink(link);
    let block = content_block_from_acp(&acp);
    match block {
        ContentBlock::Text { text } => {
            assert!(text.contains("README"), "missing name: {text}");
            assert!(
                text.contains("file:///project/README.md"),
                "missing uri: {text}"
            );
        }
        _ => panic!("expected Text"),
    }
}

#[test]
fn acp_resource_link_same_name_uri_no_duplicate() {
    let link =
        agent_client_protocol::schema::v1::ResourceLink::new("file:///x.txt", "file:///x.txt");
    let acp = AcpContentBlock::ResourceLink(link);
    let block = content_block_from_acp(&acp);
    match block {
        ContentBlock::Text { text } => {
            // Should show URI once, not duplicated
            assert_eq!(text, "[link: file:///x.txt]");
        }
        _ => panic!("expected Text"),
    }
}

// ── ToolCall conversion ─────────────────────────────────────────────

#[test]
fn acp_tool_call_converts_to_tool_use() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-1"), "Read file")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::Completed);
    let block = tool_use_from_acp(&tc);
    match block {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-1");
            assert_eq!(name, "Read file");
        }
        _ => panic!("expected ToolUse"),
    }
}

#[test]
fn acp_tool_call_undefined_title_falls_back_to_kind_label() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-u"), "undefined").kind(ToolKind::Fetch);
    let block = tool_use_from_acp(&tc);
    match block {
        ContentBlock::ToolUse { name, .. } => {
            assert_eq!(name, "WebFetch", "should fall back to kind label");
        }
        _ => panic!("expected ToolUse"),
    }
}

#[test]
fn acp_tool_call_quoted_undefined_title_falls_back() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-qu"), "\"undefined\"").kind(ToolKind::Read);
    let block = tool_use_from_acp(&tc);
    match block {
        ContentBlock::ToolUse { name, .. } => {
            assert_eq!(name, "Read", "quoted 'undefined' should fall back");
        }
        _ => panic!("expected ToolUse"),
    }
}

#[test]
fn acp_tool_call_null_title_falls_back() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-n"), "null").kind(ToolKind::Execute);
    let block = tool_use_from_acp(&tc);
    match block {
        ContentBlock::ToolUse { name, .. } => {
            assert_eq!(name, "Bash", "null title should fall back");
        }
        _ => panic!("expected ToolUse"),
    }
}

#[test]
fn acp_tool_call_empty_title_falls_back() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-e"), "").kind(ToolKind::Search);
    let block = tool_use_from_acp(&tc);
    match block {
        ContentBlock::ToolUse { name, .. } => {
            assert_eq!(name, "Search");
        }
        _ => panic!("expected ToolUse"),
    }
}

#[test]
fn tool_kind_labels_are_correct() {
    assert_eq!(tool_kind_label(&ToolKind::Read), "Read");
    assert_eq!(tool_kind_label(&ToolKind::Edit), "Edit");
    assert_eq!(tool_kind_label(&ToolKind::Execute), "Bash");
    assert_eq!(tool_kind_label(&ToolKind::Search), "Search");
    assert_eq!(tool_kind_label(&ToolKind::Think), "Think");
    assert_eq!(tool_kind_label(&ToolKind::Fetch), "WebFetch");
    assert_eq!(tool_kind_label(&ToolKind::Delete), "Delete");
    assert_eq!(tool_kind_label(&ToolKind::Move), "Move");
    assert_eq!(tool_kind_label(&ToolKind::SwitchMode), "SwitchMode");
    assert_eq!(tool_kind_label(&ToolKind::Other), "Tool");
}

// ── SessionUpdate → AgentMessage ────────────────────────────────────

#[test]
fn agent_message_chunk_maps_to_assistant() {
    let chunk = ContentChunk::new(AcpContentBlock::Text(TextContent::new("thinking...")));
    let update = SessionUpdate::AgentMessageChunk(chunk);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Assistant {
            message,
            session_id,
            ..
        } => {
            assert_eq!(message.content.len(), 1);
            assert!(
                matches!(&message.content[0], ContentBlock::Text { text } if text == "thinking...")
            );
            assert_eq!(session_id.as_ref().unwrap().as_str(), "test-session-1");
        }
        _ => panic!("expected Assistant"),
    }
}

#[test]
fn user_message_chunk_maps_to_user() {
    let chunk = ContentChunk::new(AcpContentBlock::Text(TextContent::new("fix the bug")));
    let update = SessionUpdate::UserMessageChunk(chunk);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::User {
            message,
            session_id,
            ..
        } => {
            match &message.content {
                Some(UserBody::String(s)) => assert_eq!(s, "fix the bug"),
                _ => panic!("expected String body"),
            }
            assert_eq!(session_id.as_ref().unwrap().as_str(), "test-session-1");
        }
        _ => panic!("expected User"),
    }
}

#[test]
fn agent_thought_chunk_maps_to_thinking_block() {
    let chunk = ContentChunk::new(AcpContentBlock::Text(TextContent::new("let me reason")));
    let update = SessionUpdate::AgentThoughtChunk(chunk);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Assistant { message, .. } => {
            assert_eq!(message.content.len(), 1);
            assert!(
                matches!(&message.content[0], ContentBlock::Thinking { thinking, .. } if thinking == "let me reason")
            );
        }
        _ => panic!("expected Assistant with Thinking"),
    }
}

#[test]
fn tool_call_update_in_progress_maps_to_stream_event() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-2"), "Write file")
        .kind(ToolKind::Edit)
        .status(ToolCallStatus::InProgress);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // In-progress updates only produce a StreamEvent
    assert_eq!(msgs.len(), 1);
    assert!(matches!(&msgs[0], AgentMessage::StreamEvent { uuid, .. } if uuid == "tc-2"));
}

#[test]
fn tool_call_update_completed_with_content_emits_tool_result() {
    // ToolCallContent::Content wraps a tool_call Content struct.
    // Use .into() since ToolCallContent implements From<ContentBlock>.
    let text_block = AcpContentBlock::Text(agent_client_protocol::schema::v1::TextContent::new(
        "fn main() {}",
    ));
    let tc = AcpToolCall::new(ToolCallId::new("tc-done"), "Read main.rs")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::Completed)
        .content(vec![text_block.into()]);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // Should produce StreamEvent + tool_result User message
    assert_eq!(msgs.len(), 2);
    assert!(matches!(&msgs[0], AgentMessage::StreamEvent { .. }));
    match &msgs[1] {
        AgentMessage::User { message, .. } => match &message.content {
            Some(UserBody::Blocks(blocks)) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id, "tc-done");
                        assert_eq!(content.as_deref(), Some("fn main() {}"));
                        assert_eq!(*is_error, Some(false));
                    }
                    _ => panic!("Expected ToolResult"),
                }
            }
            _ => panic!("Expected Blocks content"),
        },
        _ => panic!("Expected User message"),
    }
}

#[test]
fn tool_call_update_failed_marks_is_error() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-fail"), "Bash")
        .kind(ToolKind::Execute)
        .status(ToolCallStatus::Failed)
        .raw_output(serde_json::Value::String("exit code 1".to_string()));
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 2);
    match &msgs[1] {
        AgentMessage::User { message, .. } => match &message.content {
            Some(UserBody::Blocks(blocks)) => match &blocks[0] {
                ContentBlock::ToolResult {
                    is_error, content, ..
                } => {
                    assert_eq!(*is_error, Some(true));
                    assert_eq!(content.as_deref(), Some("exit code 1"));
                }
                _ => panic!("Expected ToolResult"),
            },
            _ => panic!("Expected Blocks"),
        },
        _ => panic!("Expected User message"),
    }
}

#[test]
fn tool_call_update_completed_without_content_no_tool_result() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-empty"), "Think")
        .kind(ToolKind::Think)
        .status(ToolCallStatus::Completed);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // No content → only the StreamEvent, no tool_result
    assert_eq!(msgs.len(), 1);
    assert!(matches!(&msgs[0], AgentMessage::StreamEvent { .. }));
}

#[test]
fn tool_call_update_undefined_output_filtered() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-undef"), "WebFetch")
        .kind(ToolKind::Fetch)
        .status(ToolCallStatus::Completed)
        .raw_output(serde_json::Value::String("undefined".to_string()));
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // Bogus "undefined" output should be filtered → no tool_result
    assert_eq!(
        msgs.len(),
        1,
        "undefined output should not produce tool_result"
    );
    assert!(matches!(&msgs[0], AgentMessage::StreamEvent { .. }));
}

#[test]
fn tool_call_update_null_output_filtered() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-null"), "Read")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::Completed)
        .raw_output(serde_json::Value::Null);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // Null output should be filtered → no tool_result
    assert_eq!(msgs.len(), 1, "null output should not produce tool_result");
}

#[test]
fn tool_call_update_real_output_preserved() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-real"), "Read")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::Completed)
        .raw_output(serde_json::Value::String("fn main() {}".to_string()));
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // Real output should produce tool_result
    assert_eq!(msgs.len(), 2, "real output should produce tool_result");
}

#[test]
fn tool_call_update_propagates_raw_input() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-inp"), "Read")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::InProgress)
        .raw_input(serde_json::json!({"file_path": "src/main.rs"}));
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    match &msgs[0] {
        AgentMessage::StreamEvent { event, .. } => {
            let input = event.get("raw_input").expect("raw_input in stream event");
            assert_eq!(input["file_path"], "src/main.rs");
        }
        _ => panic!("Expected StreamEvent"),
    }
}

#[test]
fn tool_call_update_propagates_valid_title() {
    let mut tc = AcpToolCall::new(ToolCallId::new("tc-title"), "Read")
        .kind(ToolKind::Read)
        .status(ToolCallStatus::InProgress);
    tc.title = "Read src/main.rs".to_string();
    let mut tcu = ToolCallUpdate::from(tc);
    tcu.fields.title = Some("Read src/main.rs".to_string());
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    match &msgs[0] {
        AgentMessage::StreamEvent { event, .. } => {
            assert_eq!(event["title"], "Read src/main.rs");
        }
        _ => panic!("Expected StreamEvent"),
    }
}

#[test]
fn tool_call_update_filters_undefined_title() {
    let mut tc = AcpToolCall::new(ToolCallId::new("tc-ut"), "undefined")
        .kind(ToolKind::Fetch)
        .status(ToolCallStatus::InProgress);
    tc.title = "undefined".to_string();
    let mut tcu = ToolCallUpdate::from(tc);
    tcu.fields.title = Some("undefined".to_string());
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    match &msgs[0] {
        AgentMessage::StreamEvent { event, .. } => {
            assert!(
                event.get("title").is_none(),
                "undefined title should be filtered"
            );
        }
        _ => panic!("Expected StreamEvent"),
    }
}

#[test]
fn tool_call_update_with_diff_includes_diffs_in_stream_event() {
    let diff = agent_client_protocol::schema::v1::Diff::new("src/main.rs", "fn main() { new() }")
        .old_text("fn main() { old() }");
    let tc = AcpToolCall::new(ToolCallId::new("tc-diff"), "Edit src/main.rs")
        .kind(ToolKind::Edit)
        .status(ToolCallStatus::Completed)
        .content(vec![ToolCallContent::Diff(diff)]);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // StreamEvent should carry diffs
    let stream_event = &msgs[0];
    match stream_event {
        AgentMessage::StreamEvent { event, .. } => {
            let diffs = event
                .get("diffs")
                .and_then(|v| v.as_array())
                .expect("diffs array");
            assert_eq!(diffs.len(), 1);
            let diff_text = diffs[0].get("text").and_then(|v| v.as_str()).unwrap();
            assert!(diff_text.contains("--- src/main.rs"), "diff has header");
            assert!(diff_text.contains("+ "), "diff has additions");
        }
        _ => panic!("expected StreamEvent"),
    }
}

#[test]
fn tool_call_maps_to_assistant_with_tool_use() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-3"), "Execute command")
        .kind(ToolKind::Execute)
        .status(ToolCallStatus::Pending);
    let update = SessionUpdate::ToolCall(tc);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Assistant { message, .. } => {
            assert_eq!(message.content.len(), 1);
            assert!(
                matches!(&message.content[0], ContentBlock::ToolUse { name, .. } if name == "Execute command")
            );
        }
        _ => panic!("expected Assistant"),
    }
}

#[test]
fn plan_maps_to_plan_message() {
    use crate::agent::message::{PlanEntryPriority as JP, PlanEntryStatus as JS};

    let plan = Plan::new(vec![
        PlanEntry::new(
            "Step 1",
            PlanEntryPriority::High,
            PlanEntryStatus::Completed,
        ),
        PlanEntry::new(
            "Step 2",
            PlanEntryPriority::Medium,
            PlanEntryStatus::InProgress,
        ),
        PlanEntry::new("Step 3", PlanEntryPriority::Low, PlanEntryStatus::Pending),
    ]);
    let update = SessionUpdate::Plan(plan);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::Plan { entries } => {
            assert_eq!(entries.len(), 3);
            assert_eq!(entries[0].content, "Step 1");
            assert_eq!(entries[0].priority, JP::High);
            assert_eq!(entries[0].status, JS::Completed);
            assert_eq!(entries[1].priority, JP::Medium);
            assert_eq!(entries[1].status, JS::InProgress);
            assert_eq!(entries[2].priority, JP::Low);
            assert_eq!(entries[2].status, JS::Pending);
        }
        _ => panic!("expected Plan"),
    }
}

// ── UsageUpdate → AgentMessage (C7.15) ────────────────────────────

#[test]
fn usage_update_maps_to_usage_message() {
    use agent_client_protocol::schema::v1::{Cost, UsageUpdate};

    let usage = UsageUpdate::new(50_000, 200_000).cost(Cost::new(0.12, "USD"));
    let update = SessionUpdate::UsageUpdate(usage);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    match &msgs[0] {
        AgentMessage::UsageUpdate {
            context_tokens_used,
            context_window_size,
            cost_usd,
        } => {
            assert_eq!(*context_tokens_used, 50_000);
            assert_eq!(*context_window_size, 200_000);
            assert_eq!(*cost_usd, Some(0.12));
        }
        _ => panic!("expected UsageUpdate"),
    }
}

#[test]
fn usage_update_without_cost() {
    use agent_client_protocol::schema::v1::UsageUpdate;

    let usage = UsageUpdate::new(10_000, 128_000);
    let update = SessionUpdate::UsageUpdate(usage);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);
    assert!(matches!(
        &msgs[0],
        AgentMessage::UsageUpdate { cost_usd: None, .. }
    ));
}

// ── PromptResponse → AgentMessage ───────────────────────────────────

#[test]
fn end_turn_maps_to_success_result() {
    let response = PromptResponse::new(StopReason::EndTurn);
    let msg = message_from_prompt_response(&response, &test_session_id(), 1500, 3, Some(0.05));
    match msg {
        AgentMessage::Result {
            subtype,
            is_error,
            num_turns,
            total_cost_usd,
            duration_ms,
            ..
        } => {
            assert_eq!(subtype, "success");
            assert!(!is_error);
            assert_eq!(num_turns, 3);
            assert_eq!(total_cost_usd, Some(0.05));
            assert_eq!(duration_ms, 1500);
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn max_tokens_maps_to_error_result() {
    let response = PromptResponse::new(StopReason::MaxTokens);
    let msg = message_from_prompt_response(&response, &test_session_id(), 1000, 1, None);
    match msg {
        AgentMessage::Result {
            subtype, is_error, ..
        } => {
            assert_eq!(subtype, "max_tokens");
            assert!(is_error);
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn max_turn_requests_maps_to_error_result() {
    let response = PromptResponse::new(StopReason::MaxTurnRequests);
    let msg = message_from_prompt_response(&response, &test_session_id(), 2000, 10, None);
    match msg {
        AgentMessage::Result {
            subtype, is_error, ..
        } => {
            assert_eq!(subtype, "max_turns");
            assert!(is_error);
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn refusal_maps_to_error_result() {
    let response = PromptResponse::new(StopReason::Refusal);
    let msg = message_from_prompt_response(&response, &test_session_id(), 500, 1, None);
    match msg {
        AgentMessage::Result {
            subtype, is_error, ..
        } => {
            assert_eq!(subtype, "refusal");
            assert!(is_error);
        }
        _ => panic!("expected Result"),
    }
}

#[test]
fn cancelled_maps_to_cancelled_message() {
    let response = PromptResponse::new(StopReason::Cancelled);
    let msg = message_from_prompt_response(&response, &test_session_id(), 800, 2, Some(0.02));
    match msg {
        AgentMessage::Cancelled {
            session_id, reason, ..
        } => {
            assert_eq!(session_id.as_str(), "test-session-1");
            assert!(reason.contains("cancelled"));
        }
        _ => panic!("expected Cancelled"),
    }
}

// ── Error conversion ────────────────────────────────────────────────

#[test]
fn acp_error_converts_to_magistr_error() {
    let err = AcpError::invalid_request();
    let magistr_err = magistr_error_from_acp(&err);
    match &magistr_err {
        MagistrError::Acp(msg) => {
            assert!(msg.contains("code"), "got: {msg}");
        }
        _ => panic!("expected Acp variant"),
    }
}

#[test]
fn acp_binary_not_found_error_has_install_suggestion() {
    let err = MagistrError::Acp("`claude` binary not found on PATH".into());
    assert!(err.suggestion().is_some());
    assert!(err.suggestion().unwrap().contains("Claude Code"));
}

// ── ToolCallStatus helpers ──────────────────────────────────────────

#[test]
fn terminal_status_detection() {
    assert!(is_terminal_status(&ToolCallStatus::Completed));
    assert!(!is_terminal_status(&ToolCallStatus::Pending));
    assert!(!is_terminal_status(&ToolCallStatus::InProgress));
}

// ── ToolCallContent conversion ──────────────────────────────────────

#[test]
fn tool_call_content_text_extracted() {
    let content = vec![ToolCallContent::Content(Content::new(
        AcpContentBlock::Text(TextContent::new("file written successfully")),
    ))];
    let text = tool_call_content_to_string(&content);
    assert_eq!(text, "file written successfully");
}

#[test]
fn tool_call_content_empty_is_empty_string() {
    let text = tool_call_content_to_string(&[]);
    assert_eq!(text, "");
}

#[test]
fn tool_call_content_multiple_items_joined() {
    let content = vec![
        ToolCallContent::Content(Content::new(AcpContentBlock::Text(TextContent::new(
            "line 1",
        )))),
        ToolCallContent::Content(Content::new(AcpContentBlock::Text(TextContent::new(
            "line 2",
        )))),
    ];
    let text = tool_call_content_to_string(&content);
    assert_eq!(text, "line 1\nline 2");
}

// ── ConfigOption conversion ─────────────────────────────────────────

#[test]
fn config_option_from_acp_select() {
    use agent_client_protocol::schema::v1::{
        SessionConfigOption as AcpOpt, SessionConfigOptionCategory as AcpCat,
        SessionConfigSelectOption,
    };

    let acp_opt = AcpOpt::select(
        "model",
        "Model",
        "claude-sonnet-4",
        vec![
            SessionConfigSelectOption::new("claude-sonnet-4", "Sonnet 4"),
            SessionConfigSelectOption::new("claude-opus-4", "Opus 4"),
        ],
    )
    .category(AcpCat::Model)
    .description("Select the AI model");

    let magistr_opt = config_option_from_acp(&acp_opt);
    assert_eq!(magistr_opt.id, "model");
    assert_eq!(magistr_opt.name, "Model");
    assert_eq!(
        magistr_opt.description.as_deref(),
        Some("Select the AI model")
    );
    assert_eq!(magistr_opt.category, Some(ConfigOptionCategory::Model));
    assert_eq!(magistr_opt.current_value, "claude-sonnet-4");
    assert_eq!(magistr_opt.options.len(), 2);
    assert_eq!(magistr_opt.options[0].value_id, "claude-sonnet-4");
    assert_eq!(magistr_opt.options[0].name, "Sonnet 4");
    assert_eq!(magistr_opt.options[1].value_id, "claude-opus-4");
    assert_eq!(magistr_opt.options[1].name, "Opus 4");
    assert_eq!(magistr_opt.current_value_name(), "Sonnet 4");
}

#[test]
fn config_option_from_acp_mode_category() {
    use agent_client_protocol::schema::v1::{
        SessionConfigOption as AcpOpt, SessionConfigOptionCategory as AcpCat,
        SessionConfigSelectOption,
    };

    let acp_opt = AcpOpt::select(
        "mode",
        "Mode",
        "code",
        vec![
            SessionConfigSelectOption::new("code", "Code"),
            SessionConfigSelectOption::new("plan", "Plan"),
        ],
    )
    .category(AcpCat::Mode);

    let magistr_opt = config_option_from_acp(&acp_opt);
    assert_eq!(magistr_opt.category, Some(ConfigOptionCategory::Mode));
}

#[test]
fn config_option_from_acp_no_category() {
    use agent_client_protocol::schema::v1::{
        SessionConfigOption as AcpOpt, SessionConfigSelectOption,
    };

    let acp_opt = AcpOpt::select(
        "custom",
        "Custom Setting",
        "a",
        vec![SessionConfigSelectOption::new("a", "Option A")],
    );

    let magistr_opt = config_option_from_acp(&acp_opt);
    assert_eq!(magistr_opt.category, None);
}

#[test]
fn config_option_update_maps_to_config_update_message() {
    use agent_client_protocol::schema::v1::{
        ConfigOptionUpdate as AcpCfgUpdate, SessionConfigOption as AcpOpt,
        SessionConfigOptionCategory as AcpCat, SessionConfigSelectOption,
    };

    let acp_update = AcpCfgUpdate::new(vec![
        AcpOpt::select(
            "model",
            "Model",
            "claude-opus-4",
            vec![
                SessionConfigSelectOption::new("claude-sonnet-4", "Sonnet 4"),
                SessionConfigSelectOption::new("claude-opus-4", "Opus 4"),
            ],
        )
        .category(AcpCat::Model),
    ]);

    let update = SessionUpdate::ConfigOptionUpdate(acp_update);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert_eq!(msgs.len(), 1);

    match &msgs[0] {
        AgentMessage::ConfigUpdate { options } => {
            assert_eq!(options.len(), 1);
            assert_eq!(options[0].id, "model");
            assert_eq!(options[0].current_value, "claude-opus-4");
            assert_eq!(options[0].current_value_name(), "Opus 4");
        }
        _ => panic!("expected ConfigUpdate, got {:?}", msgs[0]),
    }
}

#[test]
fn config_options_from_acp_batch() {
    use agent_client_protocol::schema::v1::{
        SessionConfigOption as AcpOpt, SessionConfigOptionCategory as AcpCat,
        SessionConfigSelectOption,
    };

    let acp_opts = vec![
        AcpOpt::select(
            "model",
            "Model",
            "sonnet",
            vec![SessionConfigSelectOption::new("sonnet", "Sonnet")],
        )
        .category(AcpCat::Model),
        AcpOpt::select(
            "mode",
            "Mode",
            "code",
            vec![SessionConfigSelectOption::new("code", "Code")],
        )
        .category(AcpCat::Mode),
    ];

    let magistr_opts = config_options_from_acp(&acp_opts);
    assert_eq!(magistr_opts.len(), 2);
    assert_eq!(magistr_opts[0].id, "model");
    assert_eq!(magistr_opts[1].id, "mode");
}

// ── AvailableCommandsUpdate conversion (C7.13) ────────────────────

#[test]
fn available_commands_update_maps_to_commands_update() {
    use agent_client_protocol::schema::v1::{
        AvailableCommand, AvailableCommandsUpdate, UnstructuredCommandInput,
    };

    let update = AvailableCommandsUpdate::new(vec![
        AvailableCommand::new("compact", "Compact conversation context"),
        AvailableCommand::new("web", "Search the web").input(AvailableCommandInput::Unstructured(
            UnstructuredCommandInput::new("search query"),
        )),
    ]);
    let session_update = SessionUpdate::AvailableCommandsUpdate(update);
    let msgs = messages_from_session_update(&session_update, &test_session_id());
    assert_eq!(msgs.len(), 1);

    match &msgs[0] {
        AgentMessage::CommandsUpdate { commands } => {
            assert_eq!(commands.len(), 2);
            assert_eq!(commands[0].name, "compact");
            assert_eq!(commands[0].description, "Compact conversation context");
            assert!(commands[0].hint.is_none());
            assert_eq!(commands[1].name, "web");
            assert_eq!(commands[1].hint.as_deref(), Some("search query"));
        }
        _ => panic!("expected CommandsUpdate, got {:?}", msgs[0]),
    }
}

// ── ToolUseKind conversion ──────────────────────────────────────────

#[test]
fn tool_use_kind_from_acp_maps_all_variants() {
    assert_eq!(tool_use_kind_from_acp(&ToolKind::Read), ToolUseKind::Read);
    assert_eq!(tool_use_kind_from_acp(&ToolKind::Edit), ToolUseKind::Edit);
    assert_eq!(
        tool_use_kind_from_acp(&ToolKind::Delete),
        ToolUseKind::Delete
    );
    assert_eq!(
        tool_use_kind_from_acp(&ToolKind::Execute),
        ToolUseKind::Execute
    );
    assert_eq!(
        tool_use_kind_from_acp(&ToolKind::Search),
        ToolUseKind::Search
    );
    assert_eq!(tool_use_kind_from_acp(&ToolKind::Think), ToolUseKind::Think);
    assert_eq!(tool_use_kind_from_acp(&ToolKind::Fetch), ToolUseKind::Fetch);
    assert_eq!(tool_use_kind_from_acp(&ToolKind::Other), ToolUseKind::Other);
}

// ── Location extraction ─────────────────────────────────────────────

#[test]
fn tool_call_locations_extracted() {
    use agent_client_protocol::schema::v1::ToolCallLocation;
    let tc = AcpToolCall::new(ToolCallId::new("tc-loc"), "Edit file")
        .kind(ToolKind::Edit)
        .locations(vec![
            ToolCallLocation::new("src/main.rs").line(42u32),
            ToolCallLocation::new("src/lib.rs"),
        ]);
    let block = tool_use_from_acp(&tc);
    match block {
        ContentBlock::ToolUse {
            locations, kind, ..
        } => {
            assert_eq!(locations.len(), 2);
            assert_eq!(locations[0].path, "src/main.rs");
            assert_eq!(locations[0].line, Some(42));
            assert_eq!(locations[1].path, "src/lib.rs");
            assert_eq!(locations[1].line, None);
            assert_eq!(kind, Some(ToolUseKind::Edit));
        }
        _ => panic!("expected ToolUse"),
    }
}

// ── Diff formatting ─────────────────────────────────────────────────

#[test]
fn diff_new_file_shows_additions() {
    let diff = agent_client_protocol::schema::v1::Diff::new("src/new.rs", "fn main() {}\n");
    let text = format_unified_diff(&diff);
    assert!(text.contains("--- src/new.rs"));
    assert!(text.contains("+++ src/new.rs"));
    assert!(text.contains("+ fn main() {}"));
}

#[test]
fn diff_modification_shows_changes() {
    let diff = agent_client_protocol::schema::v1::Diff::new("src/lib.rs", "fn bar() {}\n")
        .old_text("fn foo() {}\n".to_string());
    let text = format_unified_diff(&diff);
    assert!(text.contains("--- src/lib.rs"));
    assert!(text.contains("- fn foo() {}"));
    assert!(text.contains("+ fn bar() {}"));
}

#[test]
fn simple_line_diff_identical() {
    let result = simple_line_diff(&["a", "b"], &["a", "b"]);
    assert!(result.iter().all(|l| l.starts_with("  ")));
}

#[test]
fn simple_line_diff_insertion() {
    let result = simple_line_diff(&["a", "c"], &["a", "b", "c"]);
    assert!(result.contains(&"+ b".to_string()));
    assert!(result.contains(&"  a".to_string()));
    assert!(result.contains(&"  c".to_string()));
}

#[test]
fn simple_line_diff_deletion() {
    let result = simple_line_diff(&["a", "b", "c"], &["a", "c"]);
    assert!(result.contains(&"- b".to_string()));
}

// ── ToolCallContent diff rendering ──────────────────────────────────

#[test]
fn tool_call_content_diff_renders_unified() {
    let content = vec![ToolCallContent::Diff(
        agent_client_protocol::schema::v1::Diff::new("src/new.rs", "fn main() {}\n"),
    )];
    let text = tool_call_content_to_string(&content);
    assert!(text.contains("--- src/new.rs"));
    assert!(text.contains("+ fn main() {}"));
}

// ── C7.12: Terminal content resolution ──────────────────────────────

#[test]
fn tool_call_content_terminal_shows_id() {
    let content = vec![ToolCallContent::Terminal(
        agent_client_protocol::schema::v1::Terminal::new("term-abc123"),
    )];
    let text = tool_call_content_to_string(&content);
    assert_eq!(text, "[terminal: term-abc123]");
}

#[test]
fn tool_call_content_mixed_with_terminal() {
    let content = vec![
        ToolCallContent::Content(Content::new(AcpContentBlock::Text(TextContent::new(
            "running tests",
        )))),
        ToolCallContent::Terminal(agent_client_protocol::schema::v1::Terminal::new("term-42")),
    ];
    let text = tool_call_content_to_string(&content);
    assert!(text.contains("running tests"));
    assert!(text.contains("[terminal: term-42]"));
}

#[test]
fn tool_call_update_with_terminal_includes_terminal_ids() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-term"), "Bash")
        .kind(ToolKind::Execute)
        .status(ToolCallStatus::InProgress)
        .content(vec![ToolCallContent::Terminal(
            agent_client_protocol::schema::v1::Terminal::new("term-99"),
        )]);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    assert!(!msgs.is_empty());
    match &msgs[0] {
        AgentMessage::StreamEvent { event, .. } => {
            let tids = event["terminal_ids"]
                .as_array()
                .expect("terminal_ids array");
            assert_eq!(tids.len(), 1);
            assert_eq!(tids[0].as_str().unwrap(), "term-99");
        }
        _ => panic!("expected StreamEvent"),
    }
}

#[test]
fn extract_tool_call_content_with_terminal() {
    let tc = AcpToolCall::new(ToolCallId::new("tc-t2"), "Execute")
        .status(ToolCallStatus::Completed)
        .content(vec![ToolCallContent::Terminal(
            agent_client_protocol::schema::v1::Terminal::new("term-xyz"),
        )]);
    let tcu = ToolCallUpdate::from(tc);
    let update = SessionUpdate::ToolCallUpdate(tcu);
    let msgs = messages_from_session_update(&update, &test_session_id());
    // Should have StreamEvent + ToolResult with terminal marker
    let has_tool_result = msgs.iter().any(|m| {
        matches!(m, AgentMessage::User { message, .. }
            if message.content.as_ref().is_some_and(|c| match c {
                UserBody::Blocks(blocks) => blocks.iter().any(|b| matches!(b,
                    ContentBlock::ToolResult { content: Some(text), .. }
                    if text.contains("[terminal: term-xyz]")
                )),
                _ => false,
            })
        )
    });
    assert!(has_tool_result, "expected ToolResult with terminal marker");
}
