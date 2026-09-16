use super::*;
use crate::harness::fixtures;
use magistr_engine::NativeSnapshot;
use serde_json::json;

fn trace(steps: Vec<serde_json::Value>) -> Trace {
    let mut trace = Trace::default();
    trace.seed(&NativeSnapshot {
        steps: fixtures::steps_from(steps),
        ..NativeSnapshot::default()
    });
    trace
}

#[test]
fn sparse_interleaved_and_nested_membership_comes_from_branch_identity() {
    let trace = trace(vec![
        json!({"step_number":100}),
        json!({"step_number":110,"branch_id":"a","branch_name":"Use local storage?","branch_from":100}),
        json!({"step_number":111}),
        json!({"step_number":120,"branch_id":"b","branch_from":110}),
        json!({"step_number":150,"branch_id":"a","outcome":"Keep the local receipt."}),
        json!({"step_number":190,"dependencies":[150]}),
    ]);
    let questions = trace.investigations();
    assert_eq!(questions[0].members, [110, 150]);
    assert_eq!(questions[0].member_label(), "110, 150");
    assert!(
        !questions[0].contains(111),
        "numeric intervals must not imply membership"
    );
    assert!(
        questions[0].contains(190),
        "explicit dependent belongs to context"
    );
    assert_eq!(questions[0].used_by, [190]);
    assert_eq!(questions[1].from, Some(110));
    assert_eq!(trace.links(), [(0, 1), (1, 3)]);
}

#[test]
fn missing_metadata_is_explicit_and_merge_inference_is_not_claimed_as_provenance() {
    let mut trace = trace(vec![
        json!({"step_number":3}),
        json!({"step_number":40,"branch_id":"private-internal-id"}),
        json!({"step_number":90}),
    ]);
    trace.apply(&fixtures::frame(
        "think",
        "branch_status_changed",
        json!({"branch_id":"private-internal-id","status":"merged"}),
    ));
    let questions = trace.investigations();
    let question = &questions[0];
    assert_eq!(question.question, "Question not recorded");
    assert_eq!(question.outcome, None);
    assert_eq!(question.merged_near, Some(90));
    assert!(question.used_by.is_empty());
    assert_eq!(question.relationship(), "findings included near 90");
}

#[test]
fn rejected_idea_does_not_abandon_its_investigation_and_refutation_is_not_use() {
    let mut trace = trace(vec![
        json!({"step_number":7}),
        json!({"step_number":8,"branch_id":"limits","branch_from":7}),
        json!({"step_number":9,"branch_id":"limits","revises_step":8,"outcome":"Shared limits rejected."}),
        json!({"step_number":10,"dependencies":[9]}),
        json!({"step_number":11,"dependencies":[{"step":8,"relation":"refutes"}]}),
    ]);
    trace.apply(&fixtures::frame(
        "think",
        "branch_status_changed",
        json!({"branch_id":"limits","status":"merged"}),
    ));
    let questions = trace.investigations();
    assert_eq!(questions[0].status_label(), "Findings used");
    assert_eq!(questions[0].outcome, Some("Shared limits rejected."));
    assert_eq!(questions[0].used_by, [10]);
    assert_eq!(trace.steps()[1].revised_by, Some(9));
    assert_eq!(trace.steps()[4].refutes, [8]);
}

#[test]
fn later_origin_metadata_still_connects_the_first_record() {
    let trace = trace(vec![
        json!({"step_number":1}),
        json!({"step_number":50,"branch_id":"a"}),
        json!({"step_number":70,"branch_id":"a","branch_from":1}),
    ]);
    assert_eq!(trace.links(), [(0, 1)]);
    assert_eq!(trace.investigations()[0].from, Some(1));
}
