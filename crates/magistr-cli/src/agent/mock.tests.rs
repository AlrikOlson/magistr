use super::*;
use crate::agent::message::{AssistantContent, ContentBlock, SessionId};

fn assistant_msg(text: &str) -> AgentMessage {
    AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "claude-opus-4-6".to_string(),
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
async fn mock_session_receives_messages() {
    let (mut session, tx) = mock_session();

    tx.send(Ok(assistant_msg("Hello!"))).unwrap();
    tx.send(Ok(result_msg("sess-1"))).unwrap();

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Assistant { .. }));

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Result { .. }));
    assert_eq!(session.session_id().unwrap().as_str(), "sess-1");
}

#[tokio::test]
async fn mock_session_tracks_follow_ups() {
    let (session, _tx, follow_ups) = mock_session_with_followups();

    session.send_followup(&"first".into()).unwrap();
    session.send_followup(&"second".into()).unwrap();

    let ups = follow_ups.lock().unwrap();
    assert_eq!(ups.len(), 2);
    assert_eq!(ups[0], "first");
    assert_eq!(ups[1], "second");
}

#[tokio::test]
async fn mock_session_abort_is_safe() {
    let (mut session, _tx) = mock_session();
    session.abort();
    session.abort();
}

#[tokio::test]
async fn mock_session_multi_turn_flow() {
    let (mut session, tx, follow_ups) = mock_session_with_followups();

    // Turn 1
    tx.send(Ok(assistant_msg("Here's my answer"))).unwrap();
    tx.send(Ok(result_msg("sess-abc"))).unwrap();
    let _ = session.next_message().await;
    let _ = session.next_message().await;

    // User sends follow-up
    session.send_followup(&"tell me more".into()).unwrap();

    // Turn 2
    tx.send(Ok(assistant_msg("More details..."))).unwrap();
    tx.send(Ok(result_msg("sess-abc"))).unwrap();

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Assistant { .. }));

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Result { .. }));

    let ups = follow_ups.lock().unwrap();
    assert_eq!(ups.as_slice(), &["tell me more"]);
}
