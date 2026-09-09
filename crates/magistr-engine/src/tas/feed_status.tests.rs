use super::*;

#[test]
fn every_status_round_trips_through_its_payload() {
    let socket = Path::new("/tmp/magistr-x.sock");
    for status in [
        FeedStatus::Waiting,
        FeedStatus::Connected { reconnect: false },
        FeedStatus::Connected { reconnect: true },
        FeedStatus::Disconnected,
    ] {
        let payload = status.to_payload(socket);
        assert_eq!(payload["socket"], "/tmp/magistr-x.sock", "{payload}");
        assert_eq!(FeedStatus::from_value(&payload), Some(status), "{payload}");
    }
}

#[test]
fn payload_shape_is_tagged_by_status() {
    let payload = FeedStatus::Connected { reconnect: true }.to_payload(Path::new("/s"));
    assert_eq!(payload["status"], "connected");
    assert_eq!(payload["reconnect"], true);
    assert_eq!(
        FeedStatus::Waiting.to_payload(Path::new("/s"))["status"],
        "waiting"
    );
}

#[test]
fn only_connected_counts_as_available() {
    assert!(FeedStatus::Connected { reconnect: false }.is_connected());
    assert!(!FeedStatus::Connected { reconnect: true }.is_unavailable());
    assert!(FeedStatus::Waiting.is_unavailable());
    assert!(FeedStatus::Disconnected.is_unavailable());
}

#[test]
fn junk_is_not_a_status() {
    assert_eq!(
        FeedStatus::from_value(&serde_json::json!({"family": "ship"})),
        None
    );
    assert_eq!(FeedStatus::from_value(&Value::Null), None);
}
