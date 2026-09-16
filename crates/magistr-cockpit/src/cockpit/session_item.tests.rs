use super::*;
use crate::harness::fixtures;
use serde_json::json;

#[test]
fn session_timeline_orders_mixed_sources_by_instant_not_arrival_or_offset_text() {
    let mut trace = Trace::default();
    trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step": {
            "step_number": 2, "timestamp": "2026-09-15T01:00:04+01:00"
        }}),
    ));
    trace.apply(&fixtures::frame(
        "ship",
        "check_recorded",
        json!({"task_id": "test", "check": {
            "name": "tests", "timestamp": "2026-09-15T00:00:03Z", "passed": true
        }}),
    ));
    trace.apply(&fixtures::frame(
        "ship",
        "action_recorded",
        json!({"task_id": "build", "action": {
            "id": 1, "timestamp": "2026-09-15T00:00:01Z", "description": "implemented"
        }}),
    ));
    trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step": {
            "step_number": 1, "timestamp": "2026-09-15T00:00:00Z"
        }}),
    ));
    let mut changes = Changes::default();
    changes.set_entries(vec![JournalEntry {
        id: "7".into(),
        path: "src/lib.rs".into(),
        at_ms: Some(1_789_430_402_000),
        ..JournalEntry::default()
    }]);
    let rows = SessionItem::collect(&trace, &changes);
    assert!(matches!(rows[0], SessionItem::Step(_, step) if step.number == 1));
    assert!(matches!(
        rows[1],
        SessionItem::Moment(_, ShipMoment::Action { id: 1, .. })
    ));
    assert!(matches!(rows[2], SessionItem::Change(_, _)));
    assert!(matches!(
        rows[3],
        SessionItem::Moment(_, ShipMoment::Check { .. })
    ));
    assert!(matches!(rows[4], SessionItem::Step(_, step) if step.number == 2));
}

#[test]
fn session_timeline_keeps_ties_stable_and_unknown_times_explicitly_last() {
    let mut trace = Trace::default();
    for (number, timestamp) in [
        (3, ""),
        (2, "2026-09-15T00:00:00Z"),
        (1, "2026-09-15T00:00:00Z"),
    ] {
        trace.apply(&fixtures::frame(
            "think",
            "step_appended",
            json!({"step": {
                "step_number": number, "timestamp": timestamp
            }}),
        ));
    }
    let changes = Changes::default();
    let rows = SessionItem::collect(&trace, &changes);
    let numbers: Vec<_> = rows
        .iter()
        .map(|row| match row {
            SessionItem::Step(_, step) => step.number,
            _ => panic!("only steps"),
        })
        .collect();
    assert_eq!(numbers, [2, 1, 3]);
    assert_eq!(rows[2].time_label(), "time unknown");
}
