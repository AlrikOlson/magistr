use super::*;

#[test]
fn invalid_command_display() {
    let err = EngineError::invalid_command("SendFollowup", "Completed", "session has ended");
    assert_eq!(
        err.to_string(),
        "invalid command SendFollowup in state Completed: session has ended"
    );
}

#[test]
fn invalid_phase_transition_display() {
    let err = EngineError::invalid_phase_transition("Verifying", "Researching");
    assert_eq!(
        err.to_string(),
        "invalid phase transition from Verifying to Researching"
    );
}

#[test]
fn precondition_display() {
    let err = EngineError::Precondition("session ID required".into());
    assert_eq!(err.to_string(), "precondition failed: session ID required");
}

#[test]
fn errors_are_clone_and_eq() {
    let err = EngineError::Precondition("test".into());
    let cloned = err.clone();
    assert_eq!(err, cloned);
}
