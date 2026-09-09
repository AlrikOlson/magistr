use magistr_engine::NativeFrame;
use serde_json::json;

use super::*;

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> NativeFrame {
    NativeFrame {
        family: family.to_owned(),
        kind: kind.to_owned(),
        payload,
    }
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
fn engine_frames_imply_the_phase() {
    assert_eq!(
        Phase::from_frame(&frame(
            "roadmap",
            "chunk_changed",
            json!({"chunk": {"id": "a", "status": "in_progress"}})
        )),
        Some(Phase::Research)
    );
    assert_eq!(
        Phase::from_frame(&frame(
            "roadmap",
            "chunk_changed",
            json!({"chunk": {"id": "a", "status": "done"}})
        )),
        None,
        "only a chunk being started means research"
    );
    assert_eq!(
        Phase::from_frame(&frame("ship", "objective_set", json!({"objective": {}}))),
        Some(Phase::Plan)
    );
    assert_eq!(
        Phase::from_frame(&frame("ship", "task_started", json!({"task_id": "t1"}))),
        Some(Phase::Execute)
    );
    assert_eq!(
        Phase::from_frame(&frame(
            "ship",
            "check_recorded",
            json!({"task_id": "t1", "check": {"name": "test", "passed": true}})
        )),
        Some(Phase::Verify)
    );
    assert_eq!(
        Phase::from_frame(&frame("ship", "objective_shipped", json!({"warnings": []}))),
        Some(Phase::Done)
    );
}

#[test]
fn a_recorded_set_phase_action_names_the_phase_and_other_actions_do_not() {
    let action = |description: &str| {
        frame(
            "ship",
            "action_recorded",
            json!({"task_id": "t1", "action": {"description": description, "tools_used": ["magistr_set_phase"]}}),
        )
    };
    assert_eq!(
        Phase::from_frame(&action("workflow phase → verify")),
        Some(Phase::Verify)
    );
    assert_eq!(Phase::from_frame(&action("workflow phase → nowhere")), None);
    assert_eq!(Phase::from_frame(&action("wrote the tests")), None);
    assert_eq!(
        Phase::from_frame(&frame("think", "step_appended", json!({"step": {}}))),
        None
    );
    assert_eq!(
        Phase::from_frame(&frame("ship", "task_added", json!({"task_id": "t1"}))),
        None
    );
}
