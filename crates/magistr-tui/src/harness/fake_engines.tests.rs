use std::time::Duration;

use super::*;

#[test]
fn frames_carry_family_kind_and_payload() {
    let engines = FakeEngines::new().connected(Duration::ZERO).frame(
        Duration::from_millis(5),
        "ship",
        "action_recorded",
        serde_json::json!({"action": {"id": 7}}),
    );
    assert_eq!(engines.len(), 2);
    assert!(!engines.is_empty());
    let steps = engines.into_timeline().into_steps();
    assert_eq!(
        steps[0].1,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Connected {
            reconnect: false
        }))
    );
    match &steps[1].1 {
        Msg::Engine(EngineEvent::Frame(frame)) => {
            assert_eq!(frame.family, "ship");
            assert_eq!(frame.kind, "action_recorded");
            assert_eq!(frame.payload["action"]["id"], 7);
        }
        other => panic!("not a frame: {other:?}"),
    }
}

#[tokio::test]
async fn plays_onto_the_inbox() {
    let (inbox, mut rx) = Inbox::channel();
    FakeEngines::new()
        .feed(Duration::ZERO, FeedStatus::Waiting)
        .play(inbox);
    assert_eq!(
        rx.recv().await,
        Some(Msg::Engine(EngineEvent::Feed(FeedStatus::Waiting)))
    );
}
