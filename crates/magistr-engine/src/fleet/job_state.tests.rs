use super::*;

#[test]
fn only_the_reported_outcomes_are_terminal() {
    assert!(!JobState::Queued.is_terminal());
    assert!(!JobState::Claimed.is_terminal());
    assert!(JobState::Completed.is_terminal());
    assert!(JobState::Failed.is_terminal());
}

#[test]
fn the_wire_spelling_is_snake_case_both_ways() {
    for state in [
        JobState::Queued,
        JobState::Claimed,
        JobState::Completed,
        JobState::Failed,
    ] {
        let json = serde_json::to_string(&state).expect("serializes");
        assert_eq!(json, format!("\"{}\"", state.as_str()));
    }
}
