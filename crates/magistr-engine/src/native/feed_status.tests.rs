use super::*;

#[test]
fn only_connected_counts_as_available() {
    assert!(FeedStatus::Connected { reconnect: false }.is_connected());
    assert!(!FeedStatus::Connected { reconnect: true }.is_unavailable());
    assert!(FeedStatus::Waiting.is_unavailable());
    assert!(FeedStatus::Disconnected.is_unavailable());
}

#[test]
fn the_status_is_tagged_on_the_wire() {
    let connected = serde_json::to_value(FeedStatus::Connected { reconnect: true }).unwrap();
    assert_eq!(connected["status"], "connected");
    assert_eq!(connected["reconnect"], true);
    assert_eq!(
        serde_json::to_value(FeedStatus::Waiting).unwrap()["status"],
        "waiting"
    );
}
