use chrono::{TimeZone, Utc};

use super::*;
use crate::permissions::risk_level::RiskLevel;

fn entry(tool: &str, reason: &str, actor: Option<&str>) -> AuditEntry {
    AuditEntry {
        timestamp: Utc.with_ymd_and_hms(2026, 8, 28, 12, 0, 0).unwrap(),
        tool_name: tool.to_string(),
        risk: RiskLevel::Medium,
        allowed: true,
        reason: reason.to_string(),
        project: Some("magistr".to_string()),
        input_summary: None,
        actor: actor.map(String::from),
    }
}

#[test]
fn header_and_row() {
    let csv = export_csv(&[entry("Edit", "profile default", Some("dev@example.com"))]);
    let mut lines = csv.split("\r\n");
    assert_eq!(
        lines.next().unwrap(),
        "timestamp,actor,tool_name,risk,allowed,reason,project,input_summary"
    );
    let row = lines.next().unwrap();
    assert!(row.starts_with("2026-08-28T12:00:00+00:00,dev@example.com,Edit,medium,true,"));
    assert!(row.ends_with("profile default,magistr,"));
}

#[test]
fn empty_input_is_header_only() {
    let csv = export_csv(&[]);
    assert_eq!(
        csv,
        "timestamp,actor,tool_name,risk,allowed,reason,project,input_summary\r\n"
    );
}

#[test]
fn comma_and_quote_fields_are_escaped() {
    let csv = export_csv(&[entry("Bash", "matched rule \"git, push\"", None)]);
    // The reason field is quoted, embedded quotes doubled.
    assert!(csv.contains("\"matched rule \"\"git, push\"\"\""));
}

#[test]
fn newline_fields_are_quoted() {
    let mut e = entry("Bash", "ok", None);
    e.input_summary = Some("line one\nline two".to_string());
    let csv = export_csv(&[e]);
    assert!(csv.contains("\"line one\nline two\""));
}

#[test]
fn missing_actor_is_empty_field() {
    let csv = export_csv(&[entry("Read", "ok", None)]);
    let row = csv.split("\r\n").nth(1).unwrap();
    assert!(row.contains(",,Read,"));
}
