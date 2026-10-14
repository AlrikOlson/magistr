use super::*;
use magistr_cockpit::harness::fixtures;

#[test]
fn in_order_and_local_events_invalidate_without_folding_foreign_execution_data() {
    let mut snapshot = fixtures::snapshot(vec![fixtures::chunk("selected")], vec![]);
    let before = snapshot.roadmap.current().unwrap().clone();
    let event = magistr_engine::NativeEvent::Roadmap(magistr_engine::RoadmapFrame::ChunkChanged {
        chunk: fixtures::chunk("other"),
    });
    assert!(observe(
        &mut snapshot,
        EngineEvent::Event {
            event: event.clone(),
            delivery: Delivery::InOrder
        }
    ));
    assert!(snapshot.roadmap.current().is_err());
    assert_eq!(snapshot.roadmap.last_successful(), Some(&before));
    assert!(!observe(
        &mut snapshot,
        EngineEvent::Event {
            event: event.clone(),
            delivery: Delivery::Duplicate
        }
    ));
    assert_eq!(snapshot.roadmap.last_successful(), Some(&before));
    assert!(observe(&mut snapshot, EngineEvent::local(event)));
    assert!(snapshot.roadmap.current().is_err());
}

#[test]
fn refresh_invalidates_retained_observations_until_an_actual_unchanged_read() {
    let read = fixtures::snapshot(vec![fixtures::chunk("selected")], vec![]);
    let mut snapshot = read.clone();
    assert!(observe(&mut snapshot, EngineEvent::Refresh));
    assert!(snapshot.roadmap.current().is_err());
    assert!(snapshot.signals.current().is_err());
    assert_eq!(
        snapshot.roadmap.last_successful(),
        read.roadmap.last_successful()
    );
    assert!(!observe(&mut snapshot, EngineEvent::Seed(read.clone())));
    assert_eq!(snapshot, read);
}

#[test]
fn duplicates_and_ephemeral_phase_output_neither_request_reads_nor_clear_errors() {
    let mut snapshot = fixtures::snapshot(vec![fixtures::chunk("selected")], vec![]);
    snapshot.roadmap.record(Err(ReadError::new(
        ReadErrorKind::Decode,
        "Cannot read work records.",
    )));
    let before = snapshot.clone();
    let event = fixtures::frame(
        "magistr",
        "phase_set",
        serde_json::json!({"phase":"executing"}),
    );
    assert!(!observe(
        &mut snapshot,
        EngineEvent::Event {
            event: event.clone(),
            delivery: Delivery::Duplicate
        }
    ));
    assert!(!observe(
        &mut snapshot,
        EngineEvent::Event {
            event,
            delivery: Delivery::InOrder
        }
    ));
    assert_eq!(snapshot, before);
    assert!(observe(
        &mut snapshot,
        EngineEvent::Feed(magistr_engine::FeedStatus::Disconnected)
    ));
    assert_eq!(
        snapshot.roadmap, before.roadmap,
        "feed state does not hide the real read error"
    );
}
