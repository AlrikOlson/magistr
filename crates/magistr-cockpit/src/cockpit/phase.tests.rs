use magistr_workflow_contracts::NativeEvent;
use serde_json::json;

use super::*;
use crate::harness::fixtures;

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> NativeEvent {
    fixtures::frame(family, kind, payload)
}

#[test]
fn phases_are_ordered_named_and_parsed_back() {
    assert!(DisplayPhase::Research < DisplayPhase::Plan);
    assert!(DisplayPhase::Plan < DisplayPhase::Execute);
    assert!(DisplayPhase::Execute < DisplayPhase::Verify);
    assert!(DisplayPhase::Verify < DisplayPhase::Done);
    for phase in DisplayPhase::ALL {
        assert_eq!(DisplayPhase::from_name(phase.label()), Some(phase));
        assert_eq!(phase.to_string(), phase.label());
    }
    assert_eq!(
        DisplayPhase::from_name("Research"),
        Some(DisplayPhase::Research),
        "case-insensitive"
    );
    assert_eq!(
        DisplayPhase::from_name(" verify "),
        Some(DisplayPhase::Verify),
        "trimmed"
    );
    assert_eq!(DisplayPhase::from_name("ship"), None);
}

#[test]
fn engine_events_imply_the_phase() {
    assert_eq!(
        DisplayPhase::from_event(&frame(
            "roadmap",
            "chunk_changed",
            json!({"chunk": {"id": "a", "status": "in_progress"}})
        )),
        Some(DisplayPhase::Research)
    );
    assert_eq!(
        DisplayPhase::from_event(&frame(
            "roadmap",
            "chunk_changed",
            json!({"chunk": {"id": "a", "status": "done"}})
        )),
        None,
        "only a chunk being started means research"
    );
    assert_eq!(
        DisplayPhase::from_event(&frame("ship", "objective_set", json!({"objective": {}}))),
        Some(DisplayPhase::Plan)
    );
    assert_eq!(
        DisplayPhase::from_event(&frame("ship", "task_started", json!({"task_id": "t1"}))),
        Some(DisplayPhase::Execute)
    );
    assert_eq!(
        DisplayPhase::from_event(&frame(
            "ship",
            "check_recorded",
            json!({"task_id": "t1", "check": {"name": "test", "passed": true}})
        )),
        Some(DisplayPhase::Verify)
    );
    assert_eq!(
        DisplayPhase::from_event(&frame("ship", "objective_shipped", json!({"warnings": []}))),
        Some(DisplayPhase::Done)
    );
}

/// The announced phase travels typed on the host's own family; the action
/// the service also records on the trace is narrative, and no phase is
/// read out of its words — however they are spelled.
#[test]
fn the_announced_phase_is_read_typed_and_never_out_of_an_actions_words() {
    for (name, phase) in [
        ("researching", DisplayPhase::Research),
        ("planning", DisplayPhase::Plan),
        ("executing", DisplayPhase::Execute),
        ("verifying", DisplayPhase::Verify),
        ("complete", DisplayPhase::Done),
    ] {
        assert_eq!(
            DisplayPhase::from_event(&frame("magistr", "phase_set", json!({"phase": name}))),
            Some(phase)
        );
    }
    let action = |description: &str| {
        frame(
            "ship",
            "action_recorded",
            json!({"task_id": "t1", "action": {"description": description, "tools_used": ["magistr_set_phase"]}}),
        )
    };
    assert_eq!(
        DisplayPhase::from_event(&action("workflow phase → verify")),
        None
    );
    assert_eq!(DisplayPhase::from_event(&action("wrote the tests")), None);
    assert_eq!(
        DisplayPhase::from_event(&frame(
            "think",
            "step_appended",
            json!({"step": {"step_number": 1}})
        )),
        None
    );
    assert_eq!(
        DisplayPhase::from_event(&frame(
            "ship",
            "task_added",
            json!({"task_id": "t1", "title": "t"})
        )),
        None
    );
    assert_eq!(
        DisplayPhase::from_event(&frame(
            "magistr",
            "gate_output",
            json!({"gate": "test", "bytes": [1]})
        )),
        None
    );
}

#[test]
fn every_domain_phase_maps_onto_the_cockpits() {
    use magistr_domain::workflow::WorkflowPhase;
    assert_eq!(
        DisplayPhase::from(WorkflowPhase::Researching),
        DisplayPhase::Research
    );
    assert_eq!(
        DisplayPhase::from(WorkflowPhase::Planning),
        DisplayPhase::Plan
    );
    assert_eq!(
        DisplayPhase::from(WorkflowPhase::Executing),
        DisplayPhase::Execute
    );
    assert_eq!(
        DisplayPhase::from(WorkflowPhase::Verifying),
        DisplayPhase::Verify
    );
    assert_eq!(
        DisplayPhase::from(WorkflowPhase::Complete),
        DisplayPhase::Done
    );
}
