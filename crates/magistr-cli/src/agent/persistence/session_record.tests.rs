use super::*;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use chrono::TimeZone;

fn sample_record(id: &str, project: &str, cost: f64, turns: u32) -> SessionRecord {
    SessionRecord {
        session_id: id.to_string(),
        project: project.to_string(),
        workspace: project.to_string(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: HistoryOutcome::Completed,
        started_at: Utc.with_ymd_and_hms(2026, 3, 10, 9, 0, 0).unwrap(),
        ended_at: Utc.with_ymd_and_hms(2026, 3, 10, 9, 30, 0).unwrap(),
        cost_usd: Some(cost),
        turns: Some(turns),
        duration_ms: 1_800_000,
        files_changed: vec!["src/main.rs".into(), "src/lib.rs".into()],
        tags: vec![],
        summary: None,
        model: None,
        owner: None,
    }
}

#[test]
fn duration_display_seconds() {
    let mut r = sample_record("abc", "test", 1.0, 5);
    r.duration_ms = 45_000;
    assert_eq!(r.duration_display(), "45s");
}

#[test]
fn duration_display_minutes() {
    let mut r = sample_record("abc", "test", 1.0, 5);
    r.duration_ms = 125_000;
    assert_eq!(r.duration_display(), "2m 5s");
}

#[test]
fn duration_display_hours() {
    let mut r = sample_record("abc", "test", 1.0, 5);
    r.duration_ms = 7_500_000;
    assert_eq!(r.duration_display(), "2h 5m");
}

#[test]
fn cost_display_small() {
    let r = sample_record("abc", "test", 0.0042, 1);
    assert_eq!(r.cost_display(), "$0.0042");
}

#[test]
fn cost_display_normal() {
    let r = sample_record("abc", "test", 3.50, 10);
    assert_eq!(r.cost_display(), "$3.50");
}

#[test]
fn file_count() {
    let r = sample_record("abc", "test", 1.0, 5);
    assert_eq!(r.file_count(), 2);
}

#[test]
fn display_truncates_session_id() {
    let r = sample_record("abcdef1234567890", "myproject", 2.50, 7);
    let s = format!("{r}");
    assert!(s.starts_with("abcdef12"));
    assert!(s.contains("myproject"));
    assert!(s.contains("$2.50"));
    assert!(s.contains("7 turns"));
    assert!(s.contains("2 files"));
}

#[test]
fn display_short_session_id() {
    let r = sample_record("abc", "test", 1.0, 1);
    let s = format!("{r}");
    assert!(s.starts_with("abc"));
}

#[test]
fn session_record_serialization_round_trip() {
    let r = sample_record("test-id-123", "my-project", 4.20, 12);
    let json = serde_json::to_string(&r).expect("serialize");
    let r2: SessionRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(r, r2);
}

#[test]
fn absent_usage_remains_unknown_and_resume_requires_identity_and_capability() {
    let mut record = sample_record("host", "project", 1.0, 1);
    record.cost_usd = None;
    record.turns = None;
    assert_eq!(record.cost_display(), "unknown");
    assert_eq!(record.turns_display(), "unknown");
    record.provider_session_id = None;
    assert!(!record.can_resume());
    record.provider_session_id = Some("provider-id".into());
    record.resume_supported = false;
    assert!(!record.can_resume());
    record.resume_supported = true;
    assert!(record.can_resume());
}
