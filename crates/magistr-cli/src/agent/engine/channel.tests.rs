use super::*;

use crate::agent::engine::EngineClient;
use crate::agent::message::AgentMessage;
use tokio::sync::mpsc;

#[tokio::test]
async fn next_message_receives_from_channel() {
    let (tx, rx) = mpsc::unbounded_channel();
    let mut client = ChannelEngineClient::new(rx);

    tx.send(Ok(AgentMessage::System {
        subtype: "test".to_owned(),
        data: serde_json::json!({}),
    }))
    .expect("send should succeed");

    let msg = client.next_message().await;
    assert!(msg.is_some());
    assert!(msg.unwrap().is_ok());
}

#[tokio::test]
async fn next_message_returns_none_when_channel_closed() {
    let (tx, rx) = mpsc::unbounded_channel::<crate::error::Result<AgentMessage>>();
    let mut client = ChannelEngineClient::new(rx);

    drop(tx);

    let msg = client.next_message().await;
    assert!(msg.is_none());
}

#[tokio::test]
async fn send_command_is_noop() {
    let (_tx, rx) = mpsc::unbounded_channel::<crate::error::Result<AgentMessage>>();
    let client = ChannelEngineClient::new(rx);

    let result = client.send_command("hello").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn send_config_change_is_noop() {
    let (_tx, rx) = mpsc::unbounded_channel::<crate::error::Result<AgentMessage>>();
    let client = ChannelEngineClient::new(rx);

    let result = client
        .send_config_change("key".to_owned(), "val".to_owned())
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn cancel_closes_channel() {
    let (tx, rx) = mpsc::unbounded_channel::<crate::error::Result<AgentMessage>>();
    let mut client = ChannelEngineClient::new(rx);

    client.cancel();

    // Sending after cancel should fail (channel closed from receiver side)
    assert!(
        tx.send(Ok(AgentMessage::System {
            subtype: "test".to_owned(),
            data: serde_json::json!({}),
        }))
        .is_err()
    );
}
