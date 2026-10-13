use super::*;
use crate::agent::message::{AgentMessage, AssistantContent, ContentBlock, SessionId};
use crate::agent::mock::mock_session;
use crate::permissions::PermissionProfile;

#[tokio::test]
async fn workflow_turn_returns_without_reading_followup_or_waiting_for_eof() {
    let (mut session, tx, followups) =
        crate::agent::session::AgentSession::from_channel_with_followups();
    tx.send(Ok(result_msg("workflow", 1, None))).unwrap();
    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        drain_session_plain_turn(&mut session),
    )
    .await
    .expect("the loop owns continuation")
    .unwrap();
    assert!(followups.lock().unwrap().is_empty());
}

fn assistant_msg(text: &str) -> AgentMessage {
    AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test-model".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        },
        session_id: None,
    }
}

fn result_msg(session_id: &str, num_turns: u32, cost: Option<f64>) -> AgentMessage {
    AgentMessage::Result {
        subtype: "result".to_string(),
        duration_ms: 1500,
        duration_api_ms: 1200,
        is_error: false,
        num_turns,
        session_id: SessionId::new(session_id),
        total_cost_usd: cost,
        usage: None,
        result: None,
    }
}

fn error_result_msg(message: &str) -> AgentMessage {
    AgentMessage::Result {
        subtype: "result".to_string(),
        duration_ms: 500,
        duration_api_ms: 400,
        is_error: true,
        num_turns: 0,
        session_id: SessionId::new("error-session"),
        total_cost_usd: None,
        usage: None,
        result: Some(message.to_string()),
    }
}

// ── Mock agent session tests for run command ─────────────────

#[tokio::test]
async fn drain_session_turn_happy_path() {
    let (mut session, tx) = mock_session();

    tx.send(Ok(assistant_msg("Hello, world!"))).unwrap();
    tx.send(Ok(result_msg("sess-1", 1, Some(0.05)))).unwrap();

    let result = drain_session_turn(&mut session).await.unwrap();
    assert_eq!(result.text, "Hello, world!");
    assert!(!result.is_error);
    assert_eq!(result.num_turns, 1);
    assert_eq!(result.session_id, "sess-1");
    assert_eq!(result.total_cost_usd, Some(0.05));
}

#[tokio::test]
async fn drain_session_turn_collects_multiple_text_blocks() {
    let (mut session, tx) = mock_session();

    tx.send(Ok(assistant_msg("Part 1 "))).unwrap();
    tx.send(Ok(assistant_msg("Part 2 "))).unwrap();
    tx.send(Ok(assistant_msg("Part 3"))).unwrap();
    tx.send(Ok(result_msg("sess-multi", 1, None))).unwrap();

    let result = drain_session_turn(&mut session).await.unwrap();
    assert_eq!(result.text, "Part 1 Part 2 Part 3");
    assert_eq!(result.total_cost_usd, None);
}

#[tokio::test]
async fn drain_session_turn_error_result() {
    let (mut session, tx) = mock_session();

    tx.send(Ok(assistant_msg("Starting..."))).unwrap();
    tx.send(Ok(error_result_msg("something went wrong")))
        .unwrap();

    let result = drain_session_turn(&mut session).await.unwrap();
    assert!(result.is_error);
    assert_eq!(result.text, "Starting...");
    assert_eq!(result.session_id, "error-session");
}

#[tokio::test]
async fn drain_session_turn_stream_error() {
    let (mut session, tx) = mock_session();

    tx.send(Err(crate::error::MagistrError::Other(
        "connection lost".to_string(),
    )))
    .unwrap();

    let result = drain_session_turn(&mut session).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("connection lost"),
        "expected 'connection lost' in: {err_msg}"
    );
}

#[tokio::test]
async fn drain_session_turn_empty_stream() {
    let (mut session, tx) = mock_session();

    // Drop sender to close stream without sending any messages
    drop(tx);

    let error = drain_session_turn(&mut session).await.unwrap_err();
    assert!(error.to_string().contains("without a terminal response"));
}

#[tokio::test]
async fn drain_session_turn_partial_output_is_not_completion() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(assistant_msg("unfinished"))).unwrap();
    drop(tx);

    let error = drain_session_turn(&mut session).await.unwrap_err();
    assert!(error.to_string().contains("without a terminal response"));
}

#[tokio::test]
async fn drain_session_turn_cancellation_returns_without_waiting_for_eof() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(AgentMessage::Cancelled {
        session_id: SessionId::new("cancelled-session"),
        reason: "cancelled".into(),
    }))
    .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        drain_session_turn(&mut session),
    )
    .await
    .expect("terminal cancellation must settle a retained session");
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}

#[tokio::test]
async fn interactive_drain_propagates_stream_error_and_unacknowledged_eof() {
    for stream_error in [false, true] {
        let (mut session, tx) = mock_session();
        if stream_error {
            tx.send(Err(crate::error::MagistrError::Other(
                "provider died".into(),
            )))
            .unwrap();
        }
        drop(tx);
        let error = drain_session_interactive(&mut session).await.unwrap_err();
        assert!(error.to_string().contains(if stream_error {
            "provider died"
        } else {
            "without a terminal response"
        }));
    }
}

#[tokio::test]
async fn interactive_drain_cancellation_settles_a_retained_session() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(AgentMessage::Cancelled {
        session_id: SessionId::new("cancelled-session"),
        reason: "cancelled".into(),
    }))
    .unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        drain_session_interactive(&mut session),
    )
    .await
    .expect("cancelled turn must not wait for the retained session to close");
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}

#[tokio::test]
async fn drain_session_turn_skips_non_text_messages() {
    let (mut session, tx) = mock_session();

    // System message (should be skipped)
    tx.send(Ok(AgentMessage::System {
        subtype: "init".to_string(),
        data: serde_json::json!({}),
    }))
    .unwrap();

    // Assistant with tool use (text should be collected, tool use skipped)
    tx.send(Ok(AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test".to_string(),
            content: vec![
                ContentBlock::Text {
                    text: "Let me check.".to_string(),
                },
                ContentBlock::ToolUse {
                    id: "tc-1".to_string(),
                    name: "Bash".to_string(),
                    input: serde_json::json!({"command": "ls"}),
                    kind: None,
                    locations: vec![],
                    tool_name: None,
                },
            ],
        },
        session_id: None,
    }))
    .unwrap();

    tx.send(Ok(result_msg("sess-tools", 1, Some(0.02))))
        .unwrap();

    let result = drain_session_turn(&mut session).await.unwrap();
    assert_eq!(result.text, "Let me check.");
}

#[test]
fn resolve_profile_from_trust_flag_cautious() {
    let profile = resolve_permission_profile(std::path::Path::new("."), Some("cautious")).unwrap();
    assert_eq!(profile, PermissionProfile::Cautious);
}

#[test]
fn resolve_profile_from_trust_flag_yolo() {
    let profile = resolve_permission_profile(std::path::Path::new("."), Some("yolo")).unwrap();
    assert_eq!(profile, PermissionProfile::Yolo);
}

#[test]
fn resolve_profile_from_trust_flag_trusted() {
    let profile = resolve_permission_profile(std::path::Path::new("."), Some("trusted")).unwrap();
    assert_eq!(profile, PermissionProfile::Trusted);
}

#[test]
fn resolve_profile_from_trust_flag_standard() {
    let profile = resolve_permission_profile(std::path::Path::new("."), Some("standard")).unwrap();
    assert_eq!(profile, PermissionProfile::Standard);
}

#[test]
fn resolve_profile_invalid_flag_errors() {
    let result = resolve_permission_profile(std::path::Path::new("."), Some("invalid"));
    assert!(result.is_err());
}

#[test]
fn resolve_profile_none_defaults_to_standard() {
    // Without a project config, falls back to Standard
    let profile = resolve_permission_profile(std::path::Path::new("."), None).unwrap();
    assert_eq!(profile, PermissionProfile::Standard);
}

// ── ACP permission handler tests ─────────────────────────────────

// ── is_always_allow_option tests ─────────────────────────────────

// ── Protected path tests ──────────────────────────────────────────

// ── Smart auto-approve tests ──────────────────────────────────────

#[test]
fn translate_tools_copilot() {
    use crate::agent::{AuthMethod, translate_tools_for_runtime};
    let tools: Vec<String> = vec!["Bash".into(), "Read".into(), "Glob".into(), "Grep".into()];
    let translated = translate_tools_for_runtime(&tools, &AuthMethod::Copilot);
    assert!(translated.contains(&"shell(*)".to_string()));
    assert!(translated.contains(&"read".to_string()));
}

#[test]
fn translate_tools_acp_passthrough() {
    use crate::agent::{AuthMethod, translate_tools_for_runtime};
    let tools: Vec<String> = vec!["Bash".into(), "Read".into()];
    let translated = translate_tools_for_runtime(&tools, &AuthMethod::ApiKey);
    assert_eq!(translated, tools);
}
