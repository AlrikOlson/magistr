use super::*;
use crate::session::session_outcome::SessionOutcome as HistoryOutcome;
use chrono::TimeZone;
use tempfile::TempDir;

#[test]
fn readonly_history_never_creates_or_writes_a_database() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("missing/sessions.db");
    assert!(open_readonly_db(&missing).is_err());
    assert!(!dir.path().join("missing").exists());
    let path = dir.path().join("sessions.db");
    let writer = open_db(&path).unwrap();
    insert_session(&writer, &sample_record("saved", "project")).unwrap();
    let reader = open_readonly_db(&path).unwrap();
    assert_eq!(count_sessions(&reader).unwrap(), 1);
    assert!(insert_session(&reader, &sample_record("forbidden", "project")).is_err());
}

fn test_db() -> (TempDir, Connection) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let conn = open_db(&path).unwrap();
    (dir, conn)
}

fn sample_record(id: &str, project: &str) -> SessionRecord {
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
        cost_usd: Some(2.50),
        turns: Some(5),
        duration_ms: 1_800_000,
        files_changed: vec!["src/main.rs".into()],
        tags: vec![],
        summary: Some("Fixed the auth bug".into()),
        model: None,
        owner: None,
    }
}

#[test]
fn open_creates_db_and_schema() {
    let (_dir, conn) = test_db();
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 7);
}

#[test]
fn open_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("deep/nested/dir/test.db");
    let _conn = open_db(&path).unwrap();
    assert!(path.exists());
}

#[test]
fn insert_and_get_round_trip() {
    let (_dir, conn) = test_db();
    let record = sample_record("sess-001", "my-project");

    insert_session(&conn, &record).unwrap();

    let loaded = get_session(&conn, "sess-001").unwrap();
    assert!(loaded.is_some());
    let loaded = loaded.unwrap();
    assert_eq!(loaded.session_id, "sess-001");
    assert_eq!(loaded.project, "my-project");
    assert_eq!(loaded.cost_usd, Some(2.50));
    assert_eq!(loaded.turns, Some(5));
    assert_eq!(loaded.files_changed, vec!["src/main.rs"]);
    assert_eq!(loaded.summary.as_deref(), Some("Fixed the auth bug"));
}

#[test]
fn newer_settlement_updates_without_duplicate() {
    let (_dir, conn) = test_db();
    let mut record = sample_record("sess-001", "proj");
    insert_session(&conn, &record).unwrap();

    record.ended_at += chrono::Duration::seconds(1);
    record.cost_usd = Some(5.00);
    record.turns = Some(10);
    insert_session(&conn, &record).unwrap();

    let loaded = get_session(&conn, "sess-001").unwrap().unwrap();
    assert_eq!(loaded.cost_usd, Some(5.00));
    assert_eq!(loaded.turns, Some(10));
    assert_eq!(count_sessions(&conn).unwrap(), 1);
}

#[test]
fn get_nonexistent_returns_none() {
    let (_dir, conn) = test_db();
    let result = get_session(&conn, "no-such-id").unwrap();
    assert!(result.is_none());
}

#[test]
fn query_all_sessions() {
    let (_dir, conn) = test_db();
    insert_session(&conn, &sample_record("a", "proj-1")).unwrap();
    insert_session(&conn, &sample_record("b", "proj-2")).unwrap();
    insert_session(&conn, &sample_record("c", "proj-1")).unwrap();

    let all = query_sessions(&conn, &SessionFilter::new()).unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
fn query_filter_by_project() {
    let (_dir, conn) = test_db();
    insert_session(&conn, &sample_record("a", "magistr-cli")).unwrap();
    insert_session(&conn, &sample_record("b", "other-proj")).unwrap();

    let filter = SessionFilter::new().with_project("magistr");
    let results = query_sessions(&conn, &filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project, "magistr-cli");
}

#[test]
fn query_filter_by_since() {
    let (_dir, conn) = test_db();

    let mut old = sample_record("old", "proj");
    old.started_at = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
    old.ended_at = old.started_at + chrono::Duration::hours(1);
    insert_session(&conn, &old).unwrap();

    let recent = sample_record("new", "proj");
    insert_session(&conn, &recent).unwrap();

    let cutoff = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let filter = SessionFilter::new().with_since(cutoff);
    let results = query_sessions(&conn, &filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "new");
}

#[test]
fn query_filter_by_tag() {
    let (_dir, conn) = test_db();

    let mut tagged = sample_record("tagged", "proj");
    tagged.tags = vec!["auth".into(), "refactor".into()];
    insert_session(&conn, &tagged).unwrap();

    let untagged = sample_record("plain", "proj");
    insert_session(&conn, &untagged).unwrap();

    let filter = SessionFilter::new().with_tag("auth");
    let results = query_sessions(&conn, &filter).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "tagged");
}

#[test]
fn query_respects_limit() {
    let (_dir, conn) = test_db();
    for i in 0..10 {
        let mut r = sample_record(&format!("s-{i}"), "proj");
        r.started_at = Utc.with_ymd_and_hms(2026, 3, 10, 9, i, 0).unwrap();
        r.ended_at = r.started_at + chrono::Duration::minutes(1);
        insert_session(&conn, &r).unwrap();
    }

    let filter = SessionFilter::new().with_limit(3);
    let results = query_sessions(&conn, &filter).unwrap();
    assert_eq!(results.len(), 3);
    // Most recent first
    assert_eq!(results[0].session_id, "s-9");
}

#[test]
fn query_ordered_most_recent_first() {
    let (_dir, conn) = test_db();

    let mut r1 = sample_record("early", "proj");
    r1.started_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    r1.ended_at = r1.started_at + chrono::Duration::hours(1);
    insert_session(&conn, &r1).unwrap();

    let mut r2 = sample_record("late", "proj");
    r2.started_at = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    r2.ended_at = r2.started_at + chrono::Duration::hours(1);
    insert_session(&conn, &r2).unwrap();

    let results = query_sessions(&conn, &SessionFilter::new()).unwrap();
    assert_eq!(results[0].session_id, "late");
    assert_eq!(results[1].session_id, "early");
}

#[test]
fn count_sessions_empty() {
    let (_dir, conn) = test_db();
    assert_eq!(count_sessions(&conn).unwrap(), 0);
}

#[test]
fn count_sessions_after_insert() {
    let (_dir, conn) = test_db();
    insert_session(&conn, &sample_record("a", "p")).unwrap();
    insert_session(&conn, &sample_record("b", "p")).unwrap();
    assert_eq!(count_sessions(&conn).unwrap(), 2);
}

#[test]
fn purge_removes_old_sessions() {
    let (_dir, conn) = test_db();

    let mut old = sample_record("old", "proj");
    old.started_at = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    old.ended_at = old.started_at + chrono::Duration::hours(1);
    insert_session(&conn, &old).unwrap();

    // The shared fixture carries a fixed 2026-03-10 date, which ages past any
    // retention window; a "recent" session must be anchored to now.
    let mut recent = sample_record("recent", "proj");
    recent.started_at = Utc::now();
    recent.ended_at = Utc::now();
    insert_session(&conn, &recent).unwrap();

    let policy = RetentionPolicy { max_age_days: 30 };
    let purged = purge_old_sessions(&conn, &policy).unwrap();
    assert_eq!(purged, 1);
    assert_eq!(count_sessions(&conn).unwrap(), 1);

    let remaining = get_session(&conn, "recent").unwrap();
    assert!(remaining.is_some());
}

#[test]
fn purge_keeps_all_recent() {
    let (_dir, conn) = test_db();
    for id in ["a", "b"] {
        let mut record = sample_record(id, "p");
        record.started_at = Utc::now();
        record.ended_at = Utc::now();
        insert_session(&conn, &record).unwrap();
    }

    let policy = RetentionPolicy { max_age_days: 30 };
    let purged = purge_old_sessions(&conn, &policy).unwrap();
    assert_eq!(purged, 0);
    assert_eq!(count_sessions(&conn).unwrap(), 2);
}

#[test]
fn tag_session_adds_tag() {
    let (_dir, conn) = test_db();
    insert_session(&conn, &sample_record("s1", "proj")).unwrap();

    tag_session(&conn, "s1", "auth-refactor").unwrap();

    let loaded = get_session(&conn, "s1").unwrap().unwrap();
    assert_eq!(loaded.tags, vec!["auth-refactor"]);
}

#[test]
fn tag_session_no_duplicates() {
    let (_dir, conn) = test_db();
    insert_session(&conn, &sample_record("s1", "proj")).unwrap();

    tag_session(&conn, "s1", "fix").unwrap();
    tag_session(&conn, "s1", "fix").unwrap();

    let loaded = get_session(&conn, "s1").unwrap().unwrap();
    assert_eq!(loaded.tags, vec!["fix"]);
}

#[test]
fn tag_nonexistent_session_errors() {
    let (_dir, conn) = test_db();
    let result = tag_session(&conn, "nope", "tag");
    assert!(result.is_err());
}

#[test]
fn the_database_lives_in_the_captured_data_dir() {
    let inputs =
        ProjectInputs::from_lookup("/proj", |key| (key == "HOME").then(|| "/home/u".to_owned()));
    assert_eq!(
        sessions_db_path(&inputs),
        Some(PathBuf::from("/home/u/.local/share/magistr/sessions.db"))
    );
    let nowhere = ProjectInputs::from_lookup("/proj", |_| None);
    assert_eq!(sessions_db_path(&nowhere), None, "no home, no database");
}

#[test]
fn files_and_tags_round_trip_as_json() {
    let (_dir, conn) = test_db();

    let mut record = sample_record("json-test", "proj");
    record.files_changed = vec![
        "src/main.rs".into(),
        "src/lib.rs".into(),
        "tests/integration.rs".into(),
    ];
    record.tags = vec!["v1".into(), "release".into()];

    insert_session(&conn, &record).unwrap();

    let loaded = get_session(&conn, "json-test").unwrap().unwrap();
    assert_eq!(loaded.files_changed, record.files_changed);
    assert_eq!(loaded.tags, record.tags);
}

// ── Session Turn tests ──────────────────────────────────────────────────

fn sample_turn(index: u32, role: TurnRole, content: &str) -> SessionTurn {
    SessionTurn {
        turn_index: index,
        role,
        content: content.to_string(),
        tool_name: None,
        tool_id: None,
        timestamp: Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 0).unwrap(),
    }
}

#[test]
fn insert_and_query_turns_round_trip() {
    let (_dir, conn) = test_db();

    let t0 = sample_turn(0, TurnRole::User, "Fix the auth bug");
    let t1 = SessionTurn {
        turn_index: 1,
        role: TurnRole::Assistant,
        content: "I'll look into it.".into(),
        tool_name: None,
        tool_id: None,
        timestamp: Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 5).unwrap(),
    };
    let t2 = SessionTurn {
        turn_index: 2,
        role: TurnRole::ToolUse,
        content: "Reading src/auth.rs".into(),
        tool_name: Some("Read".into()),
        tool_id: Some("tool-1".into()),
        timestamp: Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 10).unwrap(),
    };

    insert_turn(&conn, "sess-replay-1", &t0).unwrap();
    insert_turn(&conn, "sess-replay-1", &t1).unwrap();
    insert_turn(&conn, "sess-replay-1", &t2).unwrap();

    let turns = query_turns(&conn, "sess-replay-1").unwrap();
    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].role, TurnRole::User);
    assert_eq!(turns[0].content, "Fix the auth bug");
    assert_eq!(turns[1].role, TurnRole::Assistant);
    assert_eq!(turns[2].role, TurnRole::ToolUse);
    assert_eq!(turns[2].tool_name.as_deref(), Some("Read"));
    assert_eq!(turns[2].tool_id.as_deref(), Some("tool-1"));
}

#[test]
fn query_turns_returns_ordered_by_index() {
    let (_dir, conn) = test_db();

    // Insert out of order
    insert_turn(&conn, "s1", &sample_turn(2, TurnRole::ToolUse, "step 3")).unwrap();
    insert_turn(&conn, "s1", &sample_turn(0, TurnRole::User, "step 1")).unwrap();
    insert_turn(&conn, "s1", &sample_turn(1, TurnRole::Assistant, "step 2")).unwrap();

    let turns = query_turns(&conn, "s1").unwrap();
    assert_eq!(turns[0].turn_index, 0);
    assert_eq!(turns[1].turn_index, 1);
    assert_eq!(turns[2].turn_index, 2);
}

#[test]
fn query_turns_isolates_sessions() {
    let (_dir, conn) = test_db();

    insert_turn(&conn, "s1", &sample_turn(0, TurnRole::User, "s1 msg")).unwrap();
    insert_turn(&conn, "s2", &sample_turn(0, TurnRole::User, "s2 msg")).unwrap();

    let s1_turns = query_turns(&conn, "s1").unwrap();
    assert_eq!(s1_turns.len(), 1);
    assert_eq!(s1_turns[0].content, "s1 msg");

    let s2_turns = query_turns(&conn, "s2").unwrap();
    assert_eq!(s2_turns.len(), 1);
    assert_eq!(s2_turns[0].content, "s2 msg");
}

#[test]
fn query_turns_empty_session() {
    let (_dir, conn) = test_db();
    let turns = query_turns(&conn, "no-such-session").unwrap();
    assert!(turns.is_empty());
}

#[test]
fn delete_turns_removes_all() {
    let (_dir, conn) = test_db();

    insert_turn(&conn, "s1", &sample_turn(0, TurnRole::User, "msg")).unwrap();
    insert_turn(&conn, "s1", &sample_turn(1, TurnRole::Assistant, "reply")).unwrap();

    let deleted = delete_turns(&conn, "s1").unwrap();
    assert_eq!(deleted, 2);

    let turns = query_turns(&conn, "s1").unwrap();
    assert!(turns.is_empty());
}

#[test]
fn count_turns_tracks_correctly() {
    let (_dir, conn) = test_db();

    assert_eq!(count_turns(&conn, "s1").unwrap(), 0);

    insert_turn(&conn, "s1", &sample_turn(0, TurnRole::User, "a")).unwrap();
    insert_turn(&conn, "s1", &sample_turn(1, TurnRole::Assistant, "b")).unwrap();

    assert_eq!(count_turns(&conn, "s1").unwrap(), 2);
}

#[test]
fn insert_turn_replaces_on_duplicate_index() {
    let (_dir, conn) = test_db();

    insert_turn(&conn, "s1", &sample_turn(0, TurnRole::User, "original")).unwrap();
    insert_turn(&conn, "s1", &sample_turn(0, TurnRole::User, "updated")).unwrap();

    let turns = query_turns(&conn, "s1").unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].content, "updated");
}

// ─── Owner ────────────────────────────────────────────────────────────

#[test]
fn owner_round_trips_through_insert_and_query() {
    let (_dir, conn) = test_db();
    let mut record = sample_record("owned", "proj");
    record.owner = Some("alice@example.dev".into());
    insert_session(&conn, &record).unwrap();

    let loaded = get_session(&conn, "owned").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("alice@example.dev"));
}

#[test]
fn query_filter_by_owner_substring() {
    let (_dir, conn) = test_db();
    let mut alice = sample_record("a", "proj");
    alice.owner = Some("alice@example.dev".into());
    let mut bob = sample_record("b", "proj");
    bob.owner = Some("bob@example.dev".into());
    insert_session(&conn, &alice).unwrap();
    insert_session(&conn, &bob).unwrap();
    insert_session(&conn, &sample_record("c", "proj")).unwrap(); // ownerless

    let results = query_sessions(&conn, &SessionFilter::new().with_owner("alice")).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].session_id, "a");
}

#[test]
fn set_session_owner_transfers_and_rejects_unknown() {
    let (_dir, conn) = test_db();
    let mut record = sample_record("s1", "proj");
    record.owner = Some("alice@example.dev".into());
    insert_session(&conn, &record).unwrap();

    set_session_owner(&conn, "s1", "bob@example.dev").unwrap();
    let loaded = get_session(&conn, "s1").unwrap().unwrap();
    assert_eq!(loaded.owner.as_deref(), Some("bob@example.dev"));

    let err = set_session_owner(&conn, "missing", "bob@example.dev");
    assert!(err.is_err());
}

#[test]
fn unsupported_session_schema_is_refused_without_mutation() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("unsupported.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version = 6;").unwrap();
    drop(conn);
    assert!(open_db(&path).is_err());
    let conn = Connection::open(&path).unwrap();
    let version: i32 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 6);
}

#[test]
fn sessions_store_runs_on_a_sqlite_with_the_wal_reset_fix() {
    // Several processes hold read-write WAL connections on sessions.db at
    // once (`run`, `sessions`, the home screen), which is exactly the
    // topology the WAL-reset advisory names; 3.51.3 is the floor.
    const WAL_RESET_FIX: i32 = 3_051_003;
    let (_dir, conn) = test_db();
    let runtime: String = conn
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .unwrap();
    assert_eq!(runtime, rusqlite::version());
    assert!(
        rusqlite::version_number() >= WAL_RESET_FIX,
        "linked SQLite {runtime} predates the WAL-reset fix"
    );
    let mode: String = conn
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn concurrent_wal_writers_and_checkpointers_leave_the_sessions_store_intact() {
    // The advisory's topology under control: several connections on one WAL
    // file, each writing and checkpointing while the others do the same.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sessions.db");
    drop(open_db(&path).unwrap());
    let workers: Vec<_> = (0..4)
        .map(|worker| {
            let path = path.clone();
            std::thread::spawn(move || {
                let conn = open_db(&path).unwrap();
                conn.busy_timeout(std::time::Duration::from_secs(10))
                    .unwrap();
                for n in 0..50 {
                    let id = format!("worker-{worker}-{n:03}");
                    insert_session(&conn, &sample_record(&id, "magistr")).unwrap();
                    if n % 4 == worker {
                        // TRUNCATE resets the WAL whenever no reader pins it;
                        // a busy answer is the honest outcome, never an error.
                        let busy: i64 = conn
                            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
                            .unwrap();
                        assert!(matches!(busy, 0 | 1));
                    }
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let conn = open_db(&path).unwrap();
    assert_eq!(count_sessions(&conn).unwrap(), 200);
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(
        get_session(&conn, "worker-3-049").unwrap().unwrap().project,
        "magistr"
    );
}

/// Concurrent first openers share one atomic current-schema creation.
#[test]
fn concurrent_opens_of_a_new_database_succeed() {
    for _ in 0..20 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.db");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let opened: Vec<_> = (0..4)
            .map(|_| {
                let (path, barrier) = (path.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    open_db(&path).map(|conn| count_sessions(&conn).unwrap())
                })
            })
            .collect();
        for handle in opened {
            let count = handle.join().unwrap();
            assert_eq!(count.expect("every opener gets the current schema"), 0);
        }
    }
}

#[test]
fn recent_history_and_since_filter_follow_the_latest_settlement_of_a_resumed_conversation() {
    let (_directory, conn) = test_db();
    let mut continued = sample_record("continued", "project");
    let mut newer = sample_record("newer", "project");
    continued.started_at = newer.started_at - chrono::Duration::days(30);
    continued.ended_at = newer.ended_at + chrono::Duration::days(1);
    newer.ended_at -= chrono::Duration::hours(1);
    insert_session(&conn, &newer).unwrap();
    insert_session(&conn, &continued).unwrap();
    let records = query_sessions(&conn, &SessionFilter::new().with_limit(1)).unwrap();
    assert_eq!(records[0].session_id, "continued");
    let records = query_sessions(
        &conn,
        &SessionFilter::new().with_since(newer.ended_at + chrono::Duration::minutes(1)),
    )
    .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].session_id, "continued");
}

#[test]
fn a_settlement_cannot_mix_facts_from_another_working_copy_or_provider() {
    let (_directory, conn) = test_db();
    let record = sample_record("host", "project");
    insert_session(&conn, &record).unwrap();
    let mut unrelated = record.clone();
    unrelated.ended_at += chrono::Duration::seconds(1);
    unrelated.workspace = "another-working-copy".into();
    assert!(insert_session(&conn, &unrelated).is_err());
    unrelated.workspace = record.workspace.clone();
    unrelated.provider = "another-agent".into();
    assert!(insert_session(&conn, &unrelated).is_err());
    assert_eq!(get_session(&conn, "host").unwrap().unwrap(), record);
}
