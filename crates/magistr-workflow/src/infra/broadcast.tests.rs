use super::*;

#[test]
fn no_subscribers_discards_before_fanout_can_race_with_registration() {
    // Leave fan-out unpolled so a later subscriber cannot hide a queued frame.
    let (tx, mut pending) = mpsc::unbounded_channel();
    let (subscriber_tx, subscribers) = watch::channel(0);
    let publisher = Broadcaster {
        tx,
        subscribers,
        sequence: Arc::new(AtomicU64::new(0)),
        epoch: None,
        execution_key: None,
    };

    publisher
        .emit(Family::Think, &serde_json::json!({"kind": "before"}))
        .unwrap();
    assert!(
        matches!(pending.try_recv(), Err(mpsc::error::TryRecvError::Empty)),
        "an event without subscribers must never enter the fan-out queue",
    );

    subscriber_tx.send(1).unwrap();
    publisher
        .emit(Family::Think, &serde_json::json!({"kind": "after"}))
        .unwrap();
    let frame: serde_json::Value = serde_json::from_str(&pending.try_recv().unwrap()).unwrap();
    assert_eq!(frame["kind"], "after");
    assert_eq!(
        frame["seq"], 2,
        "discarded events still advance the publisher sequence"
    );
}
