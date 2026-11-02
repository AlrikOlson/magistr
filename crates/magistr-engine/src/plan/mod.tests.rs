use super::*;

fn progress(checked: u32, total: u32) -> PlanProgress {
    PlanProgress {
        total_tasks: total,
        checked_tasks: checked,
        total_phases: 3,
        completed_phases: 0,
    }
}

// ── PlanProgress tests ───────────────────────────────────────────

#[test]
fn progress_completion_fraction() {
    let p = progress(5, 10);
    assert!((p.completion() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn progress_completion_zero_total() {
    let p = PlanProgress::default();
    assert_eq!(p.completion(), 0.0);
}

#[test]
fn progress_is_complete() {
    let p = progress(10, 10);
    assert!(p.is_complete());
}

#[test]
fn progress_is_not_complete() {
    let p = progress(9, 10);
    assert!(!p.is_complete());
}

#[test]
fn progress_empty_is_not_complete() {
    assert!(!PlanProgress::default().is_complete());
}

#[test]
fn progress_remaining() {
    let p = progress(3, 10);
    assert_eq!(p.remaining(), 7);
}

// ── PlanManager tests ────────────────────────────────────────────

#[test]
fn new_manager_with_progress() {
    let p = progress(5, 10);
    let mgr = PlanManager::new(p.clone());
    assert_eq!(mgr.progress(), &p);
    assert!((mgr.completion() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn empty_manager() {
    let mgr = PlanManager::empty();
    assert_eq!(mgr.completion(), 0.0);
    assert!(!mgr.is_complete());
}

#[test]
fn update_progress_emits_event_on_change() {
    let mut mgr = PlanManager::new(progress(5, 10));
    let event = mgr.update_progress(progress(7, 10));
    assert!(event.is_some());
    assert!(matches!(event.unwrap(), PlanEvent::ProgressUpdated { .. }));
}

#[test]
fn update_progress_no_event_when_unchanged() {
    let mut mgr = PlanManager::new(progress(5, 10));
    let event = mgr.update_progress(progress(5, 10));
    assert!(event.is_none());
}

#[test]
fn record_task_toggle_emits_event() {
    let mut mgr = PlanManager::new(progress(5, 10));
    let events = mgr.record_task_toggle("E1", "Write tests", true, progress(6, 10));

    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        PlanEvent::TaskToggled { phase_id, task_text, checked: true }
        if phase_id == "E1" && task_text == "Write tests"
    ));
    assert_eq!(mgr.progress().checked_tasks, 6);
}

#[test]
fn record_task_toggle_detects_phase_completion() {
    let mut mgr = PlanManager::new(progress(9, 10));
    let new_progress = PlanProgress {
        total_tasks: 10,
        checked_tasks: 10,
        total_phases: 3,
        completed_phases: 1,
    };
    let events = mgr.record_task_toggle("E1", "Last task", true, new_progress);

    assert_eq!(events.len(), 2);
    assert!(matches!(&events[0], PlanEvent::TaskToggled { .. }));
    assert!(matches!(&events[1], PlanEvent::PhaseCompleted { phase_id, .. } if phase_id == "E1"));
}

#[test]
fn record_task_added_emits_event() {
    let mut mgr = PlanManager::new(progress(5, 10));
    let event = mgr.record_task_added("E2", "New task", progress(5, 11));

    assert!(matches!(
        event,
        PlanEvent::TaskAdded { phase_id, task_text }
        if phase_id == "E2" && task_text == "New task"
    ));
    assert_eq!(mgr.progress().total_tasks, 11);
}

#[test]
fn record_task_removed_emits_event() {
    let mut mgr = PlanManager::new(progress(5, 10));
    let event = mgr.record_task_removed("E2", "Old task", progress(5, 9));

    assert!(matches!(
        event,
        PlanEvent::TaskRemoved { phase_id, task_text }
        if phase_id == "E2" && task_text == "Old task"
    ));
    assert_eq!(mgr.progress().total_tasks, 9);
}

#[test]
fn plan_event_serialization_roundtrip() {
    let event = PlanEvent::TaskToggled {
        phase_id: "E1".into(),
        task_text: "Extract types".into(),
        checked: true,
    };
    let json = serde_json::to_string(&event).expect("serialize");
    let parsed: PlanEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, parsed);
}

#[test]
fn manager_serialization_roundtrip() {
    let mgr = PlanManager::new(progress(5, 10));
    let json = serde_json::to_string(&mgr).expect("serialize");
    let parsed: PlanManager = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(mgr, parsed);
}
