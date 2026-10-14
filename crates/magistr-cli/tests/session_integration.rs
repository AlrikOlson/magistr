//! Integration tests for session storage, filtering, tagging, and retention.
//!
//! Exercises the full session lifecycle: create → store → query → tag → purge.
//! Tests cross-module interactions between `session::storage` and `session::types`.

use chrono::{Duration, Utc};
use magistr::session::session_outcome::SessionOutcome as HistoryOutcome;
use tempfile::TempDir;

use magistr::session::retention_policy::RetentionPolicy;
use magistr::session::session_filter::SessionFilter;
use magistr::session::session_record::SessionRecord;
use magistr::session::session_summary::SessionSummary;
use magistr::session::storage::{
    count_sessions, get_session, insert_session, open_db, purge_old_sessions, query_sessions,
    tag_session,
};

// ── Helpers ────────────────────────────────────────────────────────

fn test_db() -> (TempDir, rusqlite::Connection) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.db");
    let conn = open_db(&path).unwrap();
    (tmp, conn)
}

fn make_record(id: &str, project: &str, cost: f64, turns: u32) -> SessionRecord {
    let now = Utc::now();
    SessionRecord {
        purpose: magistr_application::sessions::SessionPurpose::Conversation,
        session_id: id.into(),
        name: None,
        project: project.into(),
        workspace: project.into(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: Some(HistoryOutcome::Completed),
        started_at: now - Duration::hours(1),
        updated_at: now,
        ended_at: Some(now),
        cost_usd: Some(cost),
        turns: Some(turns),
        duration_ms: 3_600_000,
        files_changed: vec!["src/main.rs".into(), "src/lib.rs".into()],
        tags: vec![],
        summary: Some(format!("Session {id}")),
        model: None,
        owner: None,
    }
}

// ── Full session lifecycle ─────────────────────────────────────────

#[test]
fn insert_query_tag_lifecycle() {
    let (_tmp, conn) = test_db();

    // Insert sessions
    let r1 = make_record("s1", "frontend", 2.50, 10);
    let r2 = make_record("s2", "backend", 5.00, 20);
    let r3 = make_record("s3", "frontend", 1.75, 5);
    insert_session(&conn, &r1).unwrap();
    insert_session(&conn, &r2).unwrap();
    insert_session(&conn, &r3).unwrap();

    assert_eq!(count_sessions(&conn).unwrap(), 3);

    // Query all
    let all = query_sessions(&conn, &SessionFilter::new().with_limit(100)).unwrap();
    assert_eq!(all.len(), 3);

    // Query by project
    let frontend = query_sessions(&conn, &SessionFilter::new().with_project("frontend")).unwrap();
    assert_eq!(frontend.len(), 2);

    // Tag a session
    tag_session(&conn, "s1", "important").unwrap();
    tag_session(&conn, "s1", "reviewed").unwrap();

    // Query by tag
    let tagged = query_sessions(&conn, &SessionFilter::new().with_tag("important")).unwrap();
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].session_id, "s1");

    // Verify tags persisted
    let s1 = get_session(&conn, "s1").unwrap().unwrap();
    assert!(s1.tags.contains(&"important".to_string()));
    assert!(s1.tags.contains(&"reviewed".to_string()));
}

// ── Session summary from queried records ───────────────────────────

#[test]
fn summary_from_stored_sessions() {
    let (_tmp, conn) = test_db();

    let r1 = make_record("s1", "project", 2.50, 10);
    let r2 = make_record("s2", "project", 3.50, 15);
    insert_session(&conn, &r1).unwrap();
    insert_session(&conn, &r2).unwrap();

    let records = query_sessions(&conn, &SessionFilter::new()).unwrap();
    let summary = SessionSummary::from_records(&records);

    assert_eq!(summary.count, 2);
    assert!((summary.total_cost - 6.0).abs() < f64::EPSILON);
    assert_eq!(summary.total_turns, 25);
    assert_eq!(summary.total_files, 4); // 2 files per session
}

// ── Retention purge ────────────────────────────────────────────────

#[test]
fn purge_removes_old_keeps_recent() {
    let (_tmp, conn) = test_db();

    let now = Utc::now();
    let old_record = SessionRecord {
        purpose: magistr_application::sessions::SessionPurpose::Conversation,
        session_id: "old-session".into(),
        name: None,
        project: "project".into(),
        workspace: "project".into(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: Some(HistoryOutcome::Completed),
        started_at: now - Duration::days(100),
        updated_at: now - Duration::days(99),
        ended_at: Some(now - Duration::days(99)),
        cost_usd: Some(1.0),
        turns: Some(5),
        duration_ms: 5000,
        files_changed: vec![],
        tags: vec![],
        summary: None,
        model: None,
        owner: None,
    };
    let recent_record = make_record("recent-session", "project", 2.0, 10);

    insert_session(&conn, &old_record).unwrap();
    insert_session(&conn, &recent_record).unwrap();
    assert_eq!(count_sessions(&conn).unwrap(), 2);

    // Purge sessions older than 90 days
    let policy = RetentionPolicy { max_age_days: 90 };
    let purged = purge_old_sessions(&conn, &policy).unwrap();
    assert_eq!(purged, 1);
    assert_eq!(count_sessions(&conn).unwrap(), 1);

    // Verify the recent one survived
    let remaining = get_session(&conn, "recent-session").unwrap();
    assert!(remaining.is_some());
    let old = get_session(&conn, "old-session").unwrap();
    assert!(old.is_none());
}

// ── Duplicate session replacement ──────────────────────────────────

#[test]
fn later_settlement_updates_one_conversation_with_immutable_identity() {
    let (_tmp, conn) = test_db();

    let mut r1 = make_record("same-id", "project-v1", 1.0, 5);
    insert_session(&conn, &r1).unwrap();

    // Update with same ID, different data
    r1.updated_at += chrono::Duration::seconds(1);
    r1.cost_usd = Some(3.0);
    r1.turns = Some(15);
    insert_session(&conn, &r1).unwrap();

    assert_eq!(count_sessions(&conn).unwrap(), 1);
    let loaded = get_session(&conn, "same-id").unwrap().unwrap();
    assert_eq!(loaded.project, "project-v1");
    assert!((loaded.cost_usd.unwrap() - 3.0).abs() < f64::EPSILON);
}

// ── Query with limit ───────────────────────────────────────────────

#[test]
fn query_respects_limit() {
    let (_tmp, conn) = test_db();

    for i in 0..10 {
        let r = make_record(&format!("s{i}"), "project", 1.0, 1);
        insert_session(&conn, &r).unwrap();
    }

    let limited = query_sessions(&conn, &SessionFilter::new().with_limit(3)).unwrap();
    assert_eq!(limited.len(), 3);
}

// ── Tag idempotency ────────────────────────────────────────────────

#[test]
fn duplicate_tag_not_added_twice() {
    let (_tmp, conn) = test_db();

    let r = make_record("s1", "project", 1.0, 5);
    insert_session(&conn, &r).unwrap();

    tag_session(&conn, "s1", "bug-fix").unwrap();
    tag_session(&conn, "s1", "bug-fix").unwrap();

    let loaded = get_session(&conn, "s1").unwrap().unwrap();
    let bug_fix_count = loaded.tags.iter().filter(|t| *t == "bug-fix").count();
    assert_eq!(bug_fix_count, 1);
}

// ── Session display formatting ─────────────────────────────────────

#[test]
fn session_record_display_and_helpers() {
    let r = make_record("abc123def456", "my-project", 2.345, 12);
    assert_eq!(r.file_count(), 2);
    assert_eq!(r.cost_display(), "$2.35");
    // Display truncates session ID
    let display = format!("{r}");
    assert!(display.contains("abc123de"));
    assert!(display.contains("my-project"));
}
