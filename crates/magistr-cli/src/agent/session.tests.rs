use super::*;
use crate::agent::message::{AssistantContent, ContentBlock};

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

fn result_msg(session_id: &str) -> AgentMessage {
    AgentMessage::Result {
        subtype: "result".to_string(),
        duration_ms: 1000,
        duration_api_ms: 900,
        is_error: false,
        num_turns: 1,
        session_id: SessionId::new(session_id),
        total_cost_usd: Some(0.01),
        usage: None,
        result: None,
    }
}

#[tokio::test]
async fn session_from_channel_receives_messages() {
    let (mut session, tx) = AgentSession::from_channel();

    tx.send(Ok(assistant_msg("Hello from test"))).unwrap();
    tx.send(Ok(result_msg("test-sess-1"))).unwrap();

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Assistant { .. }));

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Result { .. }));
    assert_eq!(session.session_id().unwrap().as_str(), "test-sess-1");
}

#[tokio::test]
async fn session_send_followup_captured() {
    let (session, _tx, follow_ups) = AgentSession::from_channel_with_followups();

    assert!(session.send_followup(&"follow up text".into()).is_ok());
    let captured = follow_ups.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0], "follow up text");
}

#[tokio::test]
async fn session_abort_is_idempotent() {
    let (mut session, _tx) = AgentSession::from_channel();

    // Multiple aborts should not panic
    session.abort();
    session.abort();
}

#[tokio::test]
async fn session_tracks_session_id_across_turns() {
    let (mut session, tx) = AgentSession::from_channel();

    assert!(session.session_id().is_none());

    // Turn 1
    tx.send(Ok(assistant_msg("turn 1"))).unwrap();
    tx.send(Ok(result_msg("sess-turn-1"))).unwrap();
    let _ = session.next_message().await;
    let _ = session.next_message().await;
    assert_eq!(session.session_id().unwrap().as_str(), "sess-turn-1");

    // Turn 2 — session_id updates
    tx.send(Ok(assistant_msg("turn 2"))).unwrap();
    tx.send(Ok(result_msg("sess-turn-2"))).unwrap();
    let _ = session.next_message().await;
    let _ = session.next_message().await;
    assert_eq!(session.session_id().unwrap().as_str(), "sess-turn-2");
}

#[tokio::test]
async fn session_returns_none_when_stream_ends() {
    let (mut session, tx) = AgentSession::from_channel();

    // Drop the sender to close the stream
    drop(tx);

    let result = session.next_message().await;
    assert!(result.is_none());
}

#[tokio::test]
async fn session_cancel_is_noop_for_channel() {
    let (mut session, _tx) = AgentSession::from_channel();
    // cancel should not panic for channel backend
    session.cancel();
}

#[test]
fn session_state_returns_none_for_channel() {
    let (session, _tx) = AgentSession::from_channel();
    assert!(session.session_state().is_none());
}

#[test]
fn acp_session_state_eq() {
    assert_eq!(AcpSessionState::Created, AcpSessionState::Created);
    assert_ne!(AcpSessionState::Created, AcpSessionState::InProgress);
    assert_ne!(AcpSessionState::Cancelled, AcpSessionState::Failed);
}

#[test]
fn cancelled_and_aborted_sessions_refuse_followup_input() {
    let (mut cancelled, _messages, captured) = AgentSession::from_channel_with_followups();
    cancelled.cancel();
    assert!(
        cancelled
            .send_followup(&"Must not start another turn".into())
            .is_err()
    );
    assert!(captured.lock().unwrap().is_empty());
    let (mut aborted, _messages) = AgentSession::from_channel();
    aborted.abort();
    assert!(
        aborted
            .send_followup(&"Must not start another turn".into())
            .is_err()
    );
}

#[tokio::test]
async fn cancelled_terminal_retains_the_session_id_for_continuation() {
    let (mut session, sender, followups) = AgentSession::from_channel_with_followups();
    session.cancel_turn().unwrap();
    sender
        .send(Ok(AgentMessage::Cancelled {
            session_id: SessionId::new("retained-after-cancel"),
            reason: "Provider confirmed cancellation".into(),
        }))
        .unwrap();
    assert!(matches!(
        session.next_message().await,
        Some(Ok(AgentMessage::Cancelled { .. }))
    ));
    assert_eq!(
        session.session_id().unwrap().as_str(),
        "retained-after-cancel"
    );
    session
        .send_followup(&"continue retained session".into())
        .unwrap();
    assert_eq!(
        followups.lock().unwrap().as_slice(),
        ["continue retained session"]
    );
}
