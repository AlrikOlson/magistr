use alloc::string::ToString;

use super::*;

// ── WorkflowCommand tests ────────────────────────────────────────────

#[test]
fn command_names() {
    assert_eq!(WorkflowCommand::StartIteration.name(), "start_iteration");
    assert_eq!(WorkflowCommand::Pause.name(), "pause");
    assert_eq!(WorkflowCommand::Resume.name(), "resume");
    assert_eq!(WorkflowCommand::Skip.name(), "skip");
    assert_eq!(WorkflowCommand::AbortLoop.name(), "abort_loop");
}

#[test]
fn command_display() {
    assert_eq!(WorkflowCommand::Pause.to_string(), "pause");
    assert_eq!(WorkflowCommand::AbortLoop.to_string(), "abort_loop");
}

#[test]
fn command_serialization_roundtrip() {
    let cmd = WorkflowCommand::Pause;
    let json = serde_json::to_string(&cmd).unwrap();
    let parsed: WorkflowCommand = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, cmd);
}
