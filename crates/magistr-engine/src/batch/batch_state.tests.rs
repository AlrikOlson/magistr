use super::*;

#[test]
fn the_three_settled_states_are_terminal_and_nothing_else_is() {
    for state in [
        BatchState::Succeeded,
        BatchState::Failed,
        BatchState::Skipped,
    ] {
        assert!(state.is_terminal(), "{}", state.as_str());
    }
    for state in [BatchState::Waiting, BatchState::Queued, BatchState::Running] {
        assert!(!state.is_terminal(), "{}", state.as_str());
    }
}

#[test]
fn a_project_holds_its_tree_from_release_until_it_settles() {
    // Queued counts: the job is out there and a node may pick it up at
    // any moment, so the tree is not free.
    assert!(BatchState::Queued.is_in_flight());
    assert!(BatchState::Running.is_in_flight());
    assert!(!BatchState::Waiting.is_in_flight());
    assert!(!BatchState::Succeeded.is_in_flight());
    assert!(!BatchState::Failed.is_in_flight());
    assert!(!BatchState::Skipped.is_in_flight());
}

#[test]
fn every_state_has_a_distinct_wire_spelling_that_round_trips() {
    let all = [
        BatchState::Waiting,
        BatchState::Queued,
        BatchState::Running,
        BatchState::Succeeded,
        BatchState::Failed,
        BatchState::Skipped,
    ];
    let spellings: std::collections::BTreeSet<&str> = all.iter().map(|s| s.as_str()).collect();
    assert_eq!(spellings.len(), all.len());

    for state in all {
        let json = serde_json::to_string(&state).expect("serialize");
        assert_eq!(json, format!("\"{}\"", state.as_str()));
        assert_eq!(
            serde_json::from_str::<BatchState>(&json).expect("parse"),
            state
        );
    }
}
