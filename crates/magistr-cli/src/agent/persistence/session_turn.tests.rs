use super::*;
use chrono::{TimeZone, Utc};

#[test]
fn turn_role_round_trip() {
    for role in [
        TurnRole::User,
        TurnRole::Assistant,
        TurnRole::ToolUse,
        TurnRole::ToolResult,
    ] {
        let s = role.as_str();
        let parsed = TurnRole::parse_role(s);
        assert_eq!(parsed, Some(role), "round-trip failed for {s}");
    }
}

#[test]
fn turn_role_from_str_unknown_returns_none() {
    assert_eq!(TurnRole::parse_role("unknown"), None);
    assert_eq!(TurnRole::parse_role(""), None);
}

#[test]
fn turn_role_labels() {
    assert_eq!(TurnRole::User.label(), "User");
    assert_eq!(TurnRole::Assistant.label(), "Assistant");
    assert_eq!(TurnRole::ToolUse.label(), "Tool Call");
    assert_eq!(TurnRole::ToolResult.label(), "Tool Result");
}

#[test]
fn session_turn_is_tool() {
    let turn = SessionTurn {
        turn_index: 0,
        role: TurnRole::ToolUse,
        content: "Reading file".into(),
        tool_name: Some("Read".into()),
        tool_id: Some("tool-1".into()),
        timestamp: Utc::now(),
    };
    assert!(turn.is_tool());

    let user_turn = SessionTurn {
        turn_index: 1,
        role: TurnRole::User,
        content: "Hello".into(),
        tool_name: None,
        tool_id: None,
        timestamp: Utc::now(),
    };
    assert!(!user_turn.is_tool());
}

#[test]
fn session_turn_is_long() {
    let short = SessionTurn {
        turn_index: 0,
        role: TurnRole::Assistant,
        content: "one line".into(),
        tool_name: None,
        tool_id: None,
        timestamp: Utc::now(),
    };
    assert!(!short.is_long(5));

    let long = SessionTurn {
        turn_index: 1,
        role: TurnRole::Assistant,
        content: "line1\nline2\nline3\nline4\nline5\nline6".into(),
        tool_name: None,
        tool_id: None,
        timestamp: Utc::now(),
    };
    assert!(long.is_long(5));
    assert!(!long.is_long(10));
}

#[test]
fn turn_role_display() {
    assert_eq!(format!("{}", TurnRole::User), "User");
    assert_eq!(format!("{}", TurnRole::ToolUse), "Tool Call");
}

#[test]
fn session_turn_serde_round_trip() {
    let turn = SessionTurn {
        turn_index: 3,
        role: TurnRole::ToolResult,
        content: "File contents here".into(),
        tool_name: Some("Read".into()),
        tool_id: Some("tool-42".into()),
        timestamp: Utc.with_ymd_and_hms(2026, 3, 20, 14, 30, 0).unwrap(),
    };
    let json = serde_json::to_string(&turn).unwrap();
    let deserialized: SessionTurn = serde_json::from_str(&json).unwrap();
    assert_eq!(turn, deserialized);
}
