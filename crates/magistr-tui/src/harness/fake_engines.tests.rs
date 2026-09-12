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
        Msg::Engine(EngineEvent::Event {
            event:
                magistr_engine::NativeEvent::Ship(magistr_engine::ShipFrame::ActionRecorded {
                    action,
                    ..
                }),
            delivery: magistr_engine::Delivery::Local,
        }) => assert_eq!(action.id, 7),
        other => panic!("not a local ship event: {other:?}"),
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
