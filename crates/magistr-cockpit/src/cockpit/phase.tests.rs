use magistr_engine::NativeEvent;
use serde_json::json;

use super::*;
use crate::harness::fixtures;

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> NativeEvent {
    fixtures::frame(family, kind, payload)
}

#[test]
fn phases_are_ordered_named_and_parsed_back() {
    assert!(Phase::Research < Phase::Plan);
    assert!(Phase::Plan < Phase::Execute);
    assert!(Phase::Execute < Phase::Verify);
    assert!(Phase::Verify < Phase::Done);
    for phase in Phase::ALL {
        assert_eq!(Phase::from_name(phase.label()), Some(phase));
        assert_eq!(phase.to_string(), phase.label());
    }
    assert_eq!(
        Phase::from_name("Research"),
        Some(Phase::Research),
        "case-insensitive"
    );
    assert_eq!(Phase::from_name(" verify "), Some(Phase::Verify), "trimmed");
    assert_eq!(Phase::from_name("ship"), None);
}

#[test]
fn engine_events_imply_the_phase() {
    assert_eq!(
        Phase::from_event(&frame(
            "roadmap",
            "chunk_changed",
            json!({"chunk": {"id": "a", "status": "in_progress"}})
        )),
        Some(Phase::Research)
    );
    assert_eq!(
        Phase::from_event(&frame(
            "roadmap",
            "chunk_changed",
            json!({"chunk": {"id": "a", "status": "done"}})
        )),
        None,
        "only a chunk being started means research"
    );
    assert_eq!(
        Phase::from_event(&frame("ship", "objective_set", json!({"objective": {}}))),
        Some(Phase::Plan)
    );
    assert_eq!(
        Phase::from_event(&frame("ship", "task_started", json!({"task_id": "t1"}))),
        Some(Phase::Execute)
    );
    assert_eq!(
        Phase::from_event(&frame(
            "ship",
            "check_recorded",
            json!({"task_id": "t1", "check": {"name": "test", "passed": true}})
        )),
        Some(Phase::Verify)
    );
    assert_eq!(
        Phase::from_event(&frame("ship", "objective_shipped", json!({"warnings": []}))),
        Some(Phase::Done)
    );
}

/// The announced phase travels typed on the host's own family; the action
/// the service also records on the trace is narrative, and no phase is
/// read out of its words — however they are spelled.
#[test]
fn the_announced_phase_is_read_typed_and_never_out_of_an_actions_words() {
    for (name, phase) in [
        ("researching", Phase::Research),
        ("planning", Phase::Plan),
        ("executing", Phase::Execute),
        ("verifying", Phase::Verify),
        ("complete", Phase::Done),
    ] {
        assert_eq!(
            Phase::from_event(&frame("magistr", "phase_set", json!({"phase": name}))),
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
    assert_eq!(Phase::from_event(&action("workflow phase → verify")), None);
    assert_eq!(Phase::from_event(&action("wrote the tests")), None);
    assert_eq!(
        Phase::from_event(&frame(
            "think",
            "step_appended",
            json!({"step": {"step_number": 1}})
        )),
        None
    );
    assert_eq!(
        Phase::from_event(&frame(
            "ship",
            "task_added",
            json!({"task_id": "t1", "title": "t"})
        )),
        None
    );
    assert_eq!(
        Phase::from_event(&frame(
            "magistr",
            "gate_output",
            json!({"gate": "test", "bytes": [1]})
        )),
        None
    );
}

#[test]
fn every_domain_phase_maps_onto_the_cockpits() {
    use magistr_engine::WorkflowPhase;
    assert_eq!(Phase::from(WorkflowPhase::Researching), Phase::Research);
    assert_eq!(Phase::from(WorkflowPhase::Planning), Phase::Plan);
    assert_eq!(Phase::from(WorkflowPhase::Executing), Phase::Execute);
    assert_eq!(Phase::from(WorkflowPhase::Verifying), Phase::Verify);
    assert_eq!(Phase::from(WorkflowPhase::Complete), Phase::Done);
}
