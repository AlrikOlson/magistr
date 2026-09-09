use super::*;
use crate::WorkflowPhase;

// ── SessionStatus tests ─────────────────────────────────────────────

#[test]
fn terminal_states() {
    assert!(SessionStatus::Completed.is_terminal());
    assert!(SessionStatus::Failed.is_terminal());
    assert!(SessionStatus::Cancelled.is_terminal());
    assert!(!SessionStatus::Created.is_terminal());
    assert!(!SessionStatus::InProgress.is_terminal());
    assert!(!SessionStatus::Awaiting.is_terminal());
}

#[test]
fn can_send_followup_only_when_awaiting() {
    assert!(SessionStatus::Awaiting.can_send_followup());
    assert!(!SessionStatus::Created.can_send_followup());
    assert!(!SessionStatus::InProgress.can_send_followup());
    assert!(!SessionStatus::Completed.can_send_followup());
}

#[test]
fn can_cancel_in_progress_or_awaiting() {
    assert!(SessionStatus::InProgress.can_cancel());
    assert!(SessionStatus::Awaiting.can_cancel());
    assert!(!SessionStatus::Created.can_cancel());
    assert!(!SessionStatus::Completed.can_cancel());
}

#[test]
fn status_display() {
    assert_eq!(SessionStatus::Created.to_string(), "Created");
    assert_eq!(SessionStatus::InProgress.to_string(), "In Progress");
    assert_eq!(SessionStatus::Awaiting.to_string(), "Awaiting");
    assert_eq!(SessionStatus::Completed.to_string(), "Completed");
    assert_eq!(SessionStatus::Failed.to_string(), "Failed");
    assert_eq!(SessionStatus::Cancelled.to_string(), "Cancelled");
}

#[test]
fn status_serialization_roundtrip() {
    let status = SessionStatus::InProgress;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"in_progress\"");
    let deserialized: SessionStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, status);
}

// ── SessionCommand tests ────────────────────────────────────────────

#[test]
fn command_names() {
    assert_eq!(
        SessionCommand::SendFollowup {
            content: String::new()
        }
        .name(),
        "SendFollowup"
    );
    assert_eq!(SessionCommand::Cancel.name(), "Cancel");
    assert_eq!(SessionCommand::Abort.name(), "Abort");
    assert_eq!(
        SessionCommand::SetConfig {
            key: String::new(),
            value: String::new()
        }
        .name(),
        "SetConfig"
    );
}

#[test]
fn command_display() {
    assert_eq!(SessionCommand::Cancel.to_string(), "Cancel");
}

// ── SessionCost tests ───────────────────────────────────────────────

#[test]
fn cost_default_is_zero() {
    let cost = SessionCost::default();
    assert_eq!(cost.total_usd, 0.0);
    assert_eq!(cost.input_tokens, 0);
    assert_eq!(cost.output_tokens, 0);
    assert_eq!(cost.turns, 0);
}

#[test]
fn cost_record_turn_accumulates() {
    let mut cost = SessionCost::default();
    cost.record_turn(0.05, 1000, 500);
    cost.record_turn(0.03, 800, 300);
    assert!((cost.total_usd - 0.08).abs() < f64::EPSILON);
    assert_eq!(cost.input_tokens, 1800);
    assert_eq!(cost.output_tokens, 800);
    assert_eq!(cost.turns, 2);
}

// ── EngineSession tests ─────────────────────────────────────────────

#[test]
fn new_session_starts_created() {
    let session = EngineSession::new("test-123");
    assert_eq!(session.session_id(), "test-123");
    assert_eq!(session.status(), SessionStatus::Created);
    assert_eq!(session.phase(), WorkflowPhase::Researching);
    assert_eq!(session.cost().turns, 0);
    assert!(session.messages().is_empty());
}

#[test]
fn set_status_transitions() {
    let mut session = EngineSession::new("s1");
    assert!(session.set_status(SessionStatus::InProgress).is_ok());
    assert_eq!(session.status(), SessionStatus::InProgress);
    assert!(session.set_status(SessionStatus::Awaiting).is_ok());
    assert_eq!(session.status(), SessionStatus::Awaiting);
}

#[test]
fn set_status_rejects_from_terminal() {
    let mut session = EngineSession::new("s1");
    session.set_status(SessionStatus::Completed).unwrap();
    let result = session.set_status(SessionStatus::InProgress);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("terminal state"));
}

#[test]
fn set_phase_updates() {
    let mut session = EngineSession::new("s1");
    session.set_phase(WorkflowPhase::Executing);
    assert_eq!(session.phase(), WorkflowPhase::Executing);
}

#[test]
fn record_turn_updates_cost() {
    let mut session = EngineSession::new("s1");
    session.record_turn(0.10, 2000, 1000);
    assert!((session.cost().total_usd - 0.10).abs() < f64::EPSILON);
    assert_eq!(session.cost().turns, 1);
}

#[test]
fn push_message_appends() {
    let mut session = EngineSession::new("s1");
    session.push_message("hello");
    session.push_message("world");
    assert_eq!(session.messages(), &["hello", "world"]);
}

// ── Command validation tests ────────────────────────────────────────

#[test]
fn validate_followup_only_when_awaiting() {
    let cmd = SessionCommand::SendFollowup {
        content: "hi".into(),
    };

    let mut session = EngineSession::new("s1");
    assert!(session.validate_command(&cmd).is_err());

    session.set_status(SessionStatus::InProgress).unwrap();
    assert!(session.validate_command(&cmd).is_err());

    session.set_status(SessionStatus::Awaiting).unwrap();
    assert!(session.validate_command(&cmd).is_ok());
}

#[test]
fn validate_cancel_requires_active_session() {
    let cmd = SessionCommand::Cancel;

    let mut session = EngineSession::new("s1");
    // Created — can't cancel
    assert!(session.validate_command(&cmd).is_err());

    session.set_status(SessionStatus::InProgress).unwrap();
    assert!(session.validate_command(&cmd).is_ok());
}

#[test]
fn validate_abort_always_allowed() {
    let cmd = SessionCommand::Abort;

    let session = EngineSession::new("s1");
    assert!(session.validate_command(&cmd).is_ok());

    // Even from terminal state
    let mut terminal = EngineSession::new("s2");
    terminal.set_status(SessionStatus::Completed).unwrap();
    assert!(terminal.validate_command(&cmd).is_ok());
}

#[test]
fn validate_permission_rejected_from_terminal() {
    let cmd = SessionCommand::ApprovePermission {
        request_id: "r1".into(),
    };

    let mut session = EngineSession::new("s1");
    session.set_status(SessionStatus::Failed).unwrap();
    assert!(session.validate_command(&cmd).is_err());
}

#[test]
fn validate_set_config_rejected_from_terminal() {
    let cmd = SessionCommand::SetConfig {
        key: "k".into(),
        value: "v".into(),
    };

    let mut session = EngineSession::new("s1");
    session.set_status(SessionStatus::Completed).unwrap();
    assert!(session.validate_command(&cmd).is_err());
}

#[test]
fn session_serialization_roundtrip() {
    let mut session = EngineSession::new("roundtrip-1");
    session.set_status(SessionStatus::InProgress).unwrap();
    session.set_phase(WorkflowPhase::Executing);
    session.record_turn(0.05, 500, 250);
    session.push_message("test message");

    let json = serde_json::to_string(&session).unwrap();
    let deserialized: EngineSession = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id(), "roundtrip-1");
    assert_eq!(deserialized.status(), SessionStatus::InProgress);
    assert_eq!(deserialized.phase(), WorkflowPhase::Executing);
    assert_eq!(deserialized.cost().turns, 1);
    assert_eq!(deserialized.messages(), &["test message"]);
}

// ── SessionEvent tests ──────────────────────────────────────────────

#[test]
fn event_serialization_roundtrip() {
    let event = SessionEvent::PhaseChanged {
        from: WorkflowPhase::Researching,
        to: WorkflowPhase::Planning,
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"phase_changed\""));
    let deserialized: SessionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, event);
}

#[test]
fn event_cost_updated() {
    let event = SessionEvent::CostUpdated {
        total_cost_usd: 1.23,
        turn_cost_usd: 0.05,
    };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: SessionEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, event);
}

#[test]
fn event_tool_lifecycle() {
    let started = SessionEvent::ToolStarted {
        tool_name: "Edit".into(),
        title: Some("Editing src/main.rs".into()),
    };
    let completed = SessionEvent::ToolCompleted {
        tool_name: "Edit".into(),
        success: true,
    };
    // Both should serialize/deserialize correctly
    let json_s = serde_json::to_string(&started).unwrap();
    let json_c = serde_json::to_string(&completed).unwrap();
    let _: SessionEvent = serde_json::from_str(&json_s).unwrap();
    let _: SessionEvent = serde_json::from_str(&json_c).unwrap();
}
