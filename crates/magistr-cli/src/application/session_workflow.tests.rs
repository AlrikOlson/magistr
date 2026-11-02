use super::*;
use magistr_cockpit::harness::fixtures;

#[test]
fn in_order_and_local_events_invalidate_without_folding_foreign_execution_data() {
    let mut snapshot = Arc::new(fixtures::snapshot(vec![fixtures::task("selected")], vec![]));
    let before = snapshot.plan.current().unwrap().clone();
    let event = magistr_engine::NativeEvent::Plan(magistr_engine::PlanFrame::TaskChanged {
        task: fixtures::task("other"),
    });
    assert!(observe(
        &mut snapshot,
        EngineEvent::Event {
            event: event.clone(),
            delivery: Delivery::InOrder
        }
    ));
    assert!(snapshot.plan.current().is_err());
    assert_eq!(snapshot.plan.last_successful(), Some(&before));
    assert!(!observe(
        &mut snapshot,
        EngineEvent::Event {
            event: event.clone(),
            delivery: Delivery::Duplicate
        }
    ));
    assert_eq!(snapshot.plan.last_successful(), Some(&before));
    assert!(observe(&mut snapshot, EngineEvent::local(event)));
    assert!(snapshot.plan.current().is_err());
}

#[test]
fn refresh_invalidates_retained_observations_until_an_actual_unchanged_read() {
    let read = fixtures::snapshot(vec![fixtures::task("selected")], vec![]);
    let mut snapshot = Arc::new(read.clone());
    assert!(observe(&mut snapshot, EngineEvent::Refresh));
    assert!(snapshot.plan.current().is_err());
    assert!(snapshot.signals.current().is_err());
    assert_eq!(snapshot.plan.last_successful(), read.plan.last_successful());
    assert!(!observe(&mut snapshot, EngineEvent::Seed(read.clone())));
    assert_eq!(snapshot.as_ref(), &read);
}

#[test]
fn duplicates_and_ephemeral_phase_output_neither_request_reads_nor_clear_errors() {
    let mut snapshot = Arc::new(fixtures::snapshot(vec![fixtures::task("selected")], vec![]));
    Arc::make_mut(&mut snapshot).plan.record(Err(ReadError::new(
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
        snapshot.plan, before.plan,
        "feed state does not hide the real read error"
    );
}

#[test]
fn shared_complete_snapshots_copy_only_when_the_workflow_observation_changes() {
    let mut task = fixtures::task("large");
    task.description = "Complete authored history. ".repeat(100_000);
    let original = Arc::new(fixtures::snapshot(vec![task], vec![]));
    let mut observed = original.clone();
    assert!(!observe(
        &mut observed,
        EngineEvent::Seed(original.as_ref().clone())
    ));
    assert!(Arc::ptr_eq(&observed, &original));
    assert!(observe(&mut observed, EngineEvent::Refresh));
    assert!(!Arc::ptr_eq(&observed, &original));
    assert!(original.plan.current().is_ok());
    assert!(observed.plan.current().is_err());
    assert_eq!(
        observed.plan.last_successful(),
        original.plan.last_successful()
    );
    let stale = observed.clone();
    assert!(observe(&mut observed, EngineEvent::Refresh));
    assert!(Arc::ptr_eq(&observed, &stale));
    assert!(!observe(
        &mut observed,
        EngineEvent::Seed(stale.as_ref().clone())
    ));
    assert!(Arc::ptr_eq(&observed, &stale));
    assert!(!observe(
        &mut observed,
        EngineEvent::Seed(original.as_ref().clone())
    ));
    assert!(observed.plan.current().is_ok());
    assert!(stale.plan.current().is_err());
}
