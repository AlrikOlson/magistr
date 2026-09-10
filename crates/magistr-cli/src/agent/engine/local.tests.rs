use super::*;
use crate::agent::engine::client::EngineClient;
use crate::agent::message::{AgentMessage, AssistantContent, ContentBlock, SessionId};
use crate::agent::session::AgentSession;

#[tokio::test]
async fn next_message_delegates_to_session() {
    let (session, tx) = AgentSession::from_channel();
    let msg = AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test".into(),
            content: vec![ContentBlock::Text {
                text: "hello".into(),
            }],
        },
        session_id: Some(SessionId::new("s1")),
    };
    tx.send(Ok(msg)).expect("send failed");
    drop(tx); // close the channel after one message

    let mut client = LocalEngineClient::new(session);

    let result = client.next_message().await;
    assert!(result.is_some());
    let msg = result.unwrap().expect("expected Ok message");
    assert!(matches!(msg, AgentMessage::Assistant { .. }));

    // Stream exhausted
    let result = client.next_message().await;
    assert!(result.is_none());
}

#[tokio::test]
async fn send_command_delegates_to_session() {
    let (session, _tx, follow_ups) = AgentSession::from_channel_with_followups();
    let client = LocalEngineClient::new(session);

    let result = client.send_command("hello agent").await;
    assert!(result.is_ok());

    let captured = follow_ups.lock().expect("lock poisoned");
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0], "hello agent");
}

#[tokio::test]
async fn query_state_returns_none_for_test_channel() {
    let (session, _tx) = AgentSession::from_channel();
    let client = LocalEngineClient::new(session);

    let state = client.query_state().await.expect("query_state failed");
    assert!(state.is_none()); // test channels don't track state
}
