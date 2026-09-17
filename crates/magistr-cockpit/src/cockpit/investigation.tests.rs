use super::*;
use crate::harness::fixtures;
use magistr_workflow_contracts::NativeSnapshot;
use magistr_workflow_contracts::{BranchStatus, ThinkBranch};
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
fn persisted_branch_metadata_restores_questions_status_and_exact_merge_provenance() {
    let snapshot = NativeSnapshot {
        steps: fixtures::steps_from(vec![
            json!({"step_number":10}),
            json!({"step_number":20,"branch_id":"receipt","outcome":"Keep one receipt per tenant."}),
            json!({"step_number":30}),
            json!({"step_number":40,"branch_id":"shared"}),
            json!({"step_number":70,"dependencies":[20]}),
        ]),
        branches: vec![
            ThinkBranch {
                id: "receipt".into(),
                name: "Can recovery stay tenant-specific?".into(),
                from_step: Some(10),
                status: BranchStatus::Merged,
                merged_into: Some(70),
            },
            ThinkBranch {
                id: "shared".into(),
                name: "Could tenants share a receipt?".into(),
                from_step: Some(20),
                status: BranchStatus::Abandoned,
                merged_into: None,
            },
        ],
        ..NativeSnapshot::default()
    };
    let mut restarted = Trace::default();
    restarted.seed(&snapshot);
    let questions = restarted.investigations();
    assert_eq!(questions.len(), 2);
    assert_eq!(questions[0].question, "Can recovery stay tenant-specific?");
    assert_eq!(questions[0].outcome, Some("Keep one receipt per tenant."));
    assert_eq!(
        questions[0].used_by,
        [70],
        "recorded targets are deduplicated"
    );
    assert_eq!(questions[0].merged_near, None);
    assert_eq!(
        questions[0].relationship(),
        "Explored after 10 · answer used in 70"
    );
    assert!(
        !questions[0].contains(30),
        "the earlier unrelated main-line step is not a merge"
    );
    assert_eq!(questions[1].status_label(), "Investigation stopped");
    assert_eq!(restarted.links(), [(0, 1), (1, 3)]);
    assert_eq!(restarted.merges(), [(1, 4)]);

    restarted.expand_step(20);
    restarted.apply(&fixtures::frame(
        "think",
        "branch_status_changed",
        json!({"branch_id":"receipt","status":"active"}),
    ));
    assert!(restarted.merges().is_empty());
    assert_eq!(restarted.branch_merge("receipt"), None);
    restarted.seed(&snapshot);
    assert_eq!(
        restarted.merges(),
        [(1, 4)],
        "a fresh archive repairs a missed status event"
    );
    assert_eq!(restarted.selected().map(|step| step.number), Some(20));
}

#[test]
fn exact_merge_targets_survive_live_events_without_substituting_missing_steps() {
    let mut trace = trace(vec![
        json!({"step_number":10}),
        json!({"step_number":20,"branch_id":"receipt","branch_from":10}),
        json!({"step_number":30}),
    ]);
    trace.expand_step(20);
    trace.apply(&fixtures::frame(
        "think",
        "branch_status_changed",
        json!({"branch_id":"receipt","status":"merged","merged_into":70}),
    ));
    assert!(
        trace.merges().is_empty(),
        "a missing exact target must not become step 30"
    );
    let question = &trace.investigations()[0];
    assert_eq!(question.used_by, [70]);
    assert_eq!(question.merged_near, None);
    assert_eq!(
        question.relationship(),
        "Explored after 10 · answer used in 70"
    );

    trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step":{"step_number":70}}),
    ));
    assert_eq!(trace.merges(), [(1, 3)]);
    assert_eq!(trace.selected().map(|step| step.number), Some(20));
    trace.apply(&fixtures::frame(
        "think",
        "branch_status_changed",
        json!({"branch_id":"receipt","status":"abandoned","merged_into":70}),
    ));
    assert!(trace.merges().is_empty());
    assert_eq!(
        trace.branch_merge("receipt"),
        None,
        "stopped investigations do not retain a stale merge"
    );

    trace.apply(&fixtures::frame(
        "think",
        "branch_status_changed",
        json!({"branch_id":"receipt","status":"merged"}),
    ));
    assert_eq!(
        trace.merges(),
        [(1, 2)],
        "legacy merged-only records keep the existing inference"
    );
    assert_eq!(trace.investigations()[0].merged_near, Some(30));
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
