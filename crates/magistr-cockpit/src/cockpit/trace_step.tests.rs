use serde_json::json;

use super::*;

#[test]
fn every_field_parses_from_a_full_step() {
    let step = TraceStep::from_step(&json!({
        "step_number": 7,
        "estimated_total": 9,
        "purpose": "hypothesis",
        "context": "the cache layer",
        "thought": "the TTL is racing the refresh",
        "outcome": "narrowed to the refresh path",
        "next_action": "read refresh()",
        "rationale": "the race window is there",
        "confidence": 0.7,
        "uncertainty_notes": "no repro yet",
        "revises_step": 4,
        "revision_reason": "wrong layer",
        "revised_by": 9,
        "is_final_step": true,
        "branch_from": 3,
        "branch_id": "b-cache",
        "branch_name": "cache line",
        "dependencies": [2, {"step": 5, "relation": "supports"}],
        "execution_ref": "task:fix-cache",
        "pinned": true,
    }))
    .expect("a numbered step parses");
    assert_eq!(step.number, 7);
    assert_eq!(step.estimated_total, 9);
    assert_eq!(step.purpose, "hypothesis");
    assert_eq!(step.context, "the cache layer");
    assert_eq!(step.thought, "the TTL is racing the refresh");
    assert_eq!(step.outcome, "narrowed to the refresh path");
    assert_eq!(step.next_action, "read refresh()");
    assert_eq!(step.rationale, "the race window is there");
    assert_eq!(step.confidence, Some(0.7));
    assert_eq!(step.uncertainty, "no repro yet");
    assert_eq!(step.revises, Some(4));
    assert_eq!(step.revised_by, Some(9));
    assert!(step.is_final);
    assert_eq!(step.branch_id.as_deref(), Some("b-cache"));
    assert_eq!(step.branch_name.as_deref(), Some("cache line"));
    assert_eq!(step.branch_from, Some(3));
    assert_eq!(step.dependencies, vec![2, 5]);
    assert_eq!(step.execution_ref.as_deref(), Some("task:fix-cache"));
    assert!(step.pinned);
}

#[test]
fn a_bare_step_number_is_enough_and_everything_else_defaults() {
    let step = TraceStep::from_step(&json!({"step_number": 1})).expect("parses");
    assert_eq!(step.number, 1);
    assert!(step.purpose.is_empty());
    assert_eq!(step.confidence, None);
    assert!(!step.pinned);
    assert!(!step.is_final);
    assert!(step.dependencies.is_empty());
    assert_eq!(step.branch_id, None);
}

#[test]
fn a_structured_next_action_reads_its_action() {
    let step = TraceStep::from_step(&json!({
        "step_number": 2,
        "next_action": {"action": "run the gates", "tool": "magistr_gate"},
    }))
    .expect("parses");
    assert_eq!(step.next_action, "run the gates");
}

#[test]
fn no_step_number_is_no_step() {
    assert_eq!(TraceStep::from_step(&json!({"purpose": "x"})), None);
    assert_eq!(TraceStep::from_step(&json!("nonsense")), None);
}
