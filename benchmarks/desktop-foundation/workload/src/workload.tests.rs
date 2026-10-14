use super::*;

#[test]
fn both_adapters_receive_the_same_full_representative_corpus() {
    let fixture = Fixture::new();
    assert_eq!(fixture.items.len(), 1_000);
    assert_eq!(fixture.messages.len(), 160);
    assert!(
        fixture
            .messages
            .iter()
            .all(|text| text.contains("東京") && text.contains("Requirements"))
    );
    assert_eq!(fixture.diff.len(), 8_000);
    let encoded = serde_json::to_vec(&fixture).expect("encode");
    let decoded: Fixture = serde_json::from_slice(&encoded).expect("decode");
    assert_eq!(decoded.messages, fixture.messages);
    assert_eq!(decoded.diff, fixture.diff);
}

#[test]
fn every_stream_is_independent_and_gaps_are_refused() {
    let mut state = StreamState::default();
    for stream in 0..8 {
        state
            .apply(&Event {
                stream,
                sequence: 0,
                sent_us: 0,
                text: "東京 ".into(),
            })
            .expect("first");
    }
    assert_eq!(state.sequences, [1; 8]);
    let gap = Event {
        stream: 2,
        sequence: 2,
        sent_us: 0,
        text: "lost ".into(),
    };
    assert!(state.apply(&gap).is_err());
    assert_eq!(state.sequences[2], 1);
    assert_eq!(state.texts[2], "東京 ");
    assert!(
        state
            .apply(&Event {
                stream: 8,
                sequence: 0,
                sent_us: 0,
                text: "".into()
            })
            .is_err()
    );
}

#[test]
fn bounded_producers_finish_and_delivery_is_accounted_for() {
    let run = StreamRun::start(3, std::time::Duration::ZERO);
    let mut state = StreamState::default();
    for event in run.receiver.iter() {
        state.apply(&event).expect("ordered");
    }
    assert_eq!(state.sequences, [3; 8]);
    assert_eq!(run.dropped(), 0);
}

#[test]
fn cancellation_stops_the_producers_and_settles_before_returning() {
    let run = StreamRun::start(1_250, Duration::from_secs(10));
    let cancellation = run.cancellation();
    run.receiver
        .recv_timeout(Duration::from_millis(100))
        .expect("first event");
    cancellation.stop();
    let settling = Instant::now();
    for event in run.receiver.iter() {
        assert!(event.sequence < 3, "producer continued after cancellation");
    }
    drop(run);
    assert!(settling.elapsed() < Duration::from_millis(100));
}
