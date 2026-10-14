use super::SessionRecord;
use super::*;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use chrono::TimeZone;

fn sample_record(id: &str, project: &str, cost: f64, turns: u32) -> SessionRecord {
    SessionRecord {
        session_id: id.to_string(),
        name: None,
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
fn summary_from_empty() {
    let s = SessionSummary::from_records(&[]);
    assert_eq!(s.count, 0);
    assert_eq!(s.total_cost, 0.0);
    assert!(s.earliest.is_none());
}

#[test]
fn summary_from_records() {
    let records = vec![
        sample_record("a", "proj", 1.50, 3),
        sample_record("b", "proj", 2.25, 5),
    ];
    let s = SessionSummary::from_records(&records);
    assert_eq!(s.count, 2);
    assert!((s.total_cost - 3.75).abs() < f64::EPSILON);
    assert_eq!(s.total_turns, 8);
    assert_eq!(s.total_files, 4);
    assert!(s.earliest.is_some());
    assert!(s.latest.is_some());
}

#[test]
fn summary_tracks_earliest_latest() {
    let mut r1 = sample_record("a", "proj", 1.0, 1);
    r1.started_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    r1.ended_at = Utc.with_ymd_and_hms(2026, 1, 1, 1, 0, 0).unwrap();

    let mut r2 = sample_record("b", "proj", 1.0, 1);
    r2.started_at = Utc.with_ymd_and_hms(2026, 3, 10, 0, 0, 0).unwrap();
    r2.ended_at = Utc.with_ymd_and_hms(2026, 3, 10, 2, 0, 0).unwrap();

    let s = SessionSummary::from_records(&[r1.clone(), r2.clone()]);
    assert_eq!(s.earliest, Some(r1.started_at));
    assert_eq!(s.latest, Some(r2.ended_at));
}
