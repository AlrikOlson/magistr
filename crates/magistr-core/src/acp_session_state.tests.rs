use super::*;

#[test]
fn states_are_distinct() {
    let states = [
        AcpSessionState::Created,
        AcpSessionState::InProgress,
        AcpSessionState::Awaiting,
        AcpSessionState::Completed,
        AcpSessionState::Failed,
        AcpSessionState::Cancelled,
    ];
    for (i, a) in states.iter().enumerate() {
        for (j, b) in states.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn copy_semantics() {
    let state = AcpSessionState::InProgress;
    let copy = state;
    assert_eq!(state, copy);
}

#[test]
fn debug_format() {
    let dbg = format!("{:?}", AcpSessionState::Awaiting);
    assert_eq!(dbg, "Awaiting");
}
