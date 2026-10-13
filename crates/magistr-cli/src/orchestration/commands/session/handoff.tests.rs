use super::*;
use crate::permissions::load_entries;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use crate::session::session_record::SessionRecord;
use chrono::TimeZone;
use std::path::PathBuf;
use tempfile::TempDir;

fn identity(name: &str, email: Option<&str>) -> Identity {
    Identity {
        name: name.to_string(),
        email: email.map(String::from),
        provider: None,
    }
}

fn test_db(dir: &TempDir) -> rusqlite::Connection {
    session::open_db(&dir.path().join("sessions.db")).unwrap()
}

fn log_path(dir: &TempDir) -> PathBuf {
    dir.path().join("permissions.log")
}

fn seed_session(conn: &rusqlite::Connection, id: &str, owner: Option<&str>) {
    let record = SessionRecord {
        session_id: id.to_string(),
        project: "proj".to_string(),
        workspace: "proj".to_string(),
        provider: "scripted".into(),
        provider_session_id: Some("provider-fixture".into()),
        resume_supported: true,
        outcome: HistoryOutcome::Completed,
        started_at: chrono::Utc.with_ymd_and_hms(2026, 3, 10, 9, 0, 0).unwrap(),
        ended_at: chrono::Utc.with_ymd_and_hms(2026, 3, 10, 9, 30, 0).unwrap(),
        cost_usd: Some(1.0),
        turns: Some(3),
        duration_ms: 1_800_000,
        files_changed: vec![],
        tags: vec![],
        summary: None,
        model: None,
        owner: owner.map(String::from),
    };
    session::insert_session(conn, &record).unwrap();
}

#[test]
fn owner_hands_off_their_own_session_and_audit_records_it() {
    let dir = TempDir::new().unwrap();
    let conn = test_db(&dir);
    let log = log_path(&dir);
    seed_session(&conn, "s1", Some("alice@example.dev"));

    let alice = identity("Alice", Some("alice@example.dev"));
    let receipt = handoff_session(
        &conn,
        "s1",
        "bob@example.dev",
        &alice,
        TeamRole::Developer,
        Some(&log),
        &AuditRetention::default(),
    )
    .unwrap();
    assert!(receipt.contains("bob@example.dev"));

    let loaded = session::get_session(&conn, "s1").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("bob@example.dev"));

    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tool_name, "session_handoff");
    assert!(entries[0].allowed);
    assert_eq!(entries[0].actor.as_deref(), Some("alice@example.dev"));
    assert!(entries[0].reason.contains("alice@example.dev"));
    assert!(entries[0].reason.contains("bob@example.dev"));
}

#[test]
fn developer_may_not_take_over_someone_elses_session() {
    let dir = TempDir::new().unwrap();
    let conn = test_db(&dir);
    let log = log_path(&dir);
    seed_session(&conn, "s1", Some("alice@example.dev"));

    let mallory = identity("Mallory", Some("mallory@example.dev"));
    let err = handoff_session(
        &conn,
        "s1",
        "mallory@example.dev",
        &mallory,
        TeamRole::Developer,
        Some(&log),
        &AuditRetention::default(),
    );
    assert!(err.is_err());

    // Ownership unchanged, nothing audited.
    let loaded = session::get_session(&conn, "s1").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("alice@example.dev"));
    assert!(!log.exists());
}

#[test]
fn admin_takes_over_any_session() {
    let dir = TempDir::new().unwrap();
    let conn = test_db(&dir);
    seed_session(&conn, "s1", Some("alice@example.dev"));

    let admin = identity("Root", Some("root@example.dev"));
    handoff_session(
        &conn,
        "s1",
        "carol@example.dev",
        &admin,
        TeamRole::Admin,
        None,
        &AuditRetention::default(),
    )
    .unwrap();

    let loaded = session::get_session(&conn, "s1").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("carol@example.dev"));
}

#[test]
fn ownership_match_is_case_insensitive() {
    let dir = TempDir::new().unwrap();
    let conn = test_db(&dir);
    seed_session(&conn, "s1", Some("Alice@Example.dev"));

    let alice = identity("Alice", Some("alice@example.dev"));
    handoff_session(
        &conn,
        "s1",
        "bob@example.dev",
        &alice,
        TeamRole::Viewer,
        None,
        &AuditRetention::default(),
    )
    .unwrap();
}

#[test]
fn unknown_session_is_an_error() {
    let dir = TempDir::new().unwrap();
    let conn = test_db(&dir);

    let alice = identity("Alice", Some("alice@example.dev"));
    let err = handoff_session(
        &conn,
        "missing",
        "bob@example.dev",
        &alice,
        TeamRole::Owner,
        None,
        &AuditRetention::default(),
    );
    assert!(err.is_err());
}

#[test]
fn retention_bounds_apply_after_the_handoff_append() {
    let dir = TempDir::new().unwrap();
    let conn = test_db(&dir);
    let log = log_path(&dir);
    seed_session(&conn, "s1", Some("alice@example.dev"));
    seed_session(&conn, "s2", Some("alice@example.dev"));

    let alice = identity("Alice", Some("alice@example.dev"));
    let retention = AuditRetention {
        max_entries: Some(1),
        ..AuditRetention::default()
    };
    for (id, next) in [("s1", "bob@example.dev"), ("s2", "carol@example.dev")] {
        handoff_session(
            &conn,
            id,
            next,
            &alice,
            TeamRole::Owner,
            Some(&log),
            &retention,
        )
        .unwrap();
    }

    // Only the newest entry survives the max_entries=1 bound.
    let entries = load_entries(&log).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].reason.contains("carol@example.dev"));
}
