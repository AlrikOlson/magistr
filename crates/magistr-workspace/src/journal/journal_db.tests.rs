use super::*;
use crate::journal::{JournalFileChange, JournalFileKind, JournalMode, JournalReader};

#[test]
fn fresh_journal_uses_retained_v2_schema() {
    let dir = tempfile::tempdir().expect("temporary journal directory");
    let path = dir.path().join("journal.db");
    let writer = JournalDb::open(JournalDbOptions::new(&path)).expect("open fresh journal");
    drop(writer);
    let db = rusqlite::Connection::open(&path).expect("read created database");
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("version");
    assert_eq!(version, 2);
    let tables: i64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('ops','op_files','blobs','undone')", [], |row| row.get(0)).expect("tables");
    assert_eq!(tables, 4);
}

fn change(before: &[u8], after: &[u8]) -> JournalOpInit {
    JournalOpInit {
        tool: "edit".to_owned(),
        args_hash: "args".to_owned(),
        session_id: "session-a".to_owned(),
        files: vec![JournalFileChange {
            kind: JournalFileKind::Write,
            path: "src/file.rs".to_owned(),
            before: FileContent::decode(before.to_vec()),
            after: FileContent::decode(after.to_vec()),
        }],
        undo_of: None,
    }
}

fn writer(dir: &tempfile::TempDir) -> JournalDb {
    JournalDb::open(JournalDbOptions::new(dir.path().join("journal.db"))).expect("journal")
}

fn table_counts(db: &Connection) -> (i64, i64, i64, i64, i64) {
    db.query_row("SELECT (SELECT count(*) FROM ops),(SELECT count(*) FROM op_files),(SELECT count(*) FROM blobs),(SELECT count(*) FROM undone),(SELECT seq FROM sqlite_sequence WHERE name='ops')", [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).expect("table counts")
}

#[test]
fn append_round_trips_binary_order_undo_links_and_restart() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let first = db.append(&change(b"text", &[0, 255, 1])).expect("first").op;
    let mut second = change(&[0, 255, 1], b"text");
    second.undo_of = Some(vec![first.seq, first.seq]);
    second.files.push(JournalFileChange {
        kind: JournalFileKind::Create,
        path: "second".to_owned(),
        before: FileContent::Text(String::new()),
        after: FileContent::Text("created".to_owned()),
    });
    let second = db.append(&second).expect("undo").op;
    assert_eq!(second.seq, first.seq + 1);
    assert_eq!(second.undo_of, Some(vec![first.seq]));
    assert_eq!(second.files[1].path, "second");
    drop(db);
    let db = writer(&dir);
    assert_eq!(db.get(first.seq).expect("get"), Some(first.clone()));
    assert_eq!(
        db.list(0).expect("list"),
        vec![first.clone(), second.clone()]
    );
    let recent = db.recent(&JournalLogQuery::default()).expect("recent");
    assert_eq!(
        recent.iter().map(|entry| entry.op.seq).collect::<Vec<_>>(),
        vec![second.seq, first.seq]
    );
    assert!(!recent[0].undoable);
    assert_eq!(recent[1].undone_by, Some(second.seq));
    assert!(
        db.undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates")
            .is_empty()
    );
}

#[test]
fn failed_append_rolls_back_sequence_files_blobs_links_and_eviction() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let original = db.append(&change(b"before", b"after")).expect("first").op;
    let before = table_counts(&db.connection());
    db.connection().execute_batch("CREATE TRIGGER reject_file BEFORE INSERT ON op_files WHEN NEW.path='reject' BEGIN SELECT RAISE(ABORT,'injected batch failure'); END").expect("fault injection");
    let mut failing = change(b"fresh before", b"fresh after");
    failing.undo_of = Some(vec![original.seq]);
    let mut rejected = failing.files[0].clone();
    rejected.path = "reject".to_owned();
    failing.files.push(rejected);
    assert!(matches!(db.append(&failing), Err(JournalError::Sqlite(_))));
    assert_eq!(table_counts(&db.connection()), before);
    assert_eq!(db.get(original.seq).expect("original"), Some(original));
    db.connection()
        .execute_batch("DROP TRIGGER reject_file")
        .expect("remove injection");
    assert_eq!(
        db.append(&change(b"next", b"value")).expect("next").op.seq,
        2
    );
}

#[test]
fn cap_evicts_by_reference_then_hash_and_protects_newest() {
    let dir = tempfile::tempdir().expect("directory");
    let a = vec![b'a'; 64];
    let b = vec![b'b'; 64];
    let c = vec![b'c'; 64];
    let compressed = zstd::stream::encode_all(a.as_slice(), 0)
        .expect("compress")
        .len();
    assert_eq!(
        compressed,
        zstd::stream::encode_all(b.as_slice(), 0)
            .expect("compress")
            .len()
    );
    let mut options = JournalDbOptions::new(dir.path().join("journal.db"));
    options.cap_bytes = (compressed * 2) as u64;
    let mut db = JournalDb::open(options).expect("journal");
    let first = db.append(&change(&a, &b)).expect("first").op;
    let newest = db.append(&change(&c, &c)).expect("newest");
    assert_eq!(newest.notes.len(), 1);
    assert!(newest.notes[0].contains("evicted 1 oldest blob(s)"));
    let first = db.get(first.seq).expect("get").expect("op");
    let file = &first.files[0];
    if file.before_hash < file.after_hash {
        assert!(file.before.is_none());
        assert!(file.after.is_some());
    } else {
        assert!(file.before.is_some());
        assert!(file.after.is_none());
    }
    assert!(newest.op.files[0].before.is_some());
    assert!(newest.op.files[0].after.is_some());
}

#[test]
fn newest_oversized_operation_reports_cap_and_remains_readable() {
    let dir = tempfile::tempdir().expect("directory");
    let mut options = JournalDbOptions::new(dir.path().join("journal.db"));
    options.cap_bytes = 0;
    let mut db = JournalDb::open(options).expect("journal");
    let first = db.append(&change(b"old", b"old")).expect("first").op;
    let second = db.append(&change(b"new", b"new")).expect("second");
    assert_eq!(second.notes.len(), 2);
    assert!(second.notes[1].contains("protected newest operation"));
    assert!(
        db.get(first.seq).expect("get").expect("op").files[0]
            .before
            .is_none()
    );
    assert_eq!(
        second.op.files[0]
            .before
            .as_ref()
            .expect("retained")
            .as_bytes(),
        b"new"
    );
    assert_eq!(
        db.undo_candidates(&UndoCandidateQuery::default())
            .expect("eligibility survives eviction")
            .len(),
        2
    );
}

#[test]
fn reuse_preserves_compressed_bytes_and_refreshes_recency() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let op = db.append(&change(b"reused", b"reused")).expect("first").op;
    let hash = &op.files[0].before_hash;
    let custom = zstd::stream::encode_all(&b"reused"[..], 19).expect("custom compression");
    db.connection()
        .execute(
            "UPDATE blobs SET zstd=?,bytes=? WHERE hash=?",
            params![custom, custom.len() as i64, hash],
        )
        .expect("retained codec variant");
    let second = db.append(&change(b"reused", b"different")).expect("second");
    let (bytes, reference): (Vec<u8>, i64) = db
        .connection()
        .query_row(
            "SELECT zstd,last_ref_seq FROM blobs WHERE hash=?",
            [hash],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("blob");
    assert_eq!(bytes, custom);
    assert_eq!(reference, second.op.seq);
}

#[test]
fn pagination_and_id_selection_keep_scope_and_eligibility() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let mut input = change(b"a", b"b");
    let first = db.append(&input).expect("first").op;
    input.session_id = "session-b".to_owned();
    let second = db.append(&input).expect("second").op;
    input.session_id = "session-a".to_owned();
    let third = db.append(&input).expect("third").op;
    let candidates = db
        .undo_candidates(&UndoCandidateQuery {
            session_id: Some("session-a".to_owned()),
            seqs: Some(vec![second.seq, third.seq, first.seq, 999]),
            limit: 100,
        })
        .expect("candidates");
    assert_eq!(
        candidates.iter().map(|op| op.seq).collect::<Vec<_>>(),
        vec![third.seq, first.seq]
    );
    let page = db
        .recent(&JournalLogQuery {
            since_seq: Some(first.seq),
            before_seq: Some(third.seq),
            session_id: Some("session-a".to_owned()),
            limit: 1,
        })
        .expect("page");
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].op.seq, first.seq);
    assert!(
        db.undo_candidates(&UndoCandidateQuery {
            seqs: Some(vec![]),
            ..Default::default()
        })
        .expect("empty ids")
        .is_empty()
    );
}

#[test]
fn readonly_connection_observes_live_writer_without_migration() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let first = db.append(&change(b"a", b"b")).expect("first").op;
    let reader = JournalReader::open(dir.path().join("journal.db")).expect("reader");
    assert_eq!(reader.get(first.seq).expect("read"), Some(first));
    let second = db.append(&change(b"b", b"c")).expect("second").op;
    assert_eq!(
        reader.get(second.seq).expect("read new commit"),
        Some(second)
    );
    assert_eq!(
        reader
            .recent(&JournalLogQuery::default())
            .expect("reader view"),
        db.recent(&JournalLogQuery::default()).expect("writer view")
    );
    assert_eq!(reader.list(0).expect("list"), db.list(0).expect("list"));
    assert_eq!(
        reader
            .undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates"),
        db.undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates")
    );
}

#[test]
fn corrupt_blob_is_an_explicit_read_error_and_append_rollback() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let first = db.append(&change(b"before", b"after")).expect("first").op;
    db.connection()
        .execute(
            "UPDATE blobs SET zstd=x'0001',bytes=2 WHERE hash=?",
            [&first.files[0].before_hash],
        )
        .expect("corrupt codec");
    let before = table_counts(&db.connection());
    assert!(matches!(
        db.get(first.seq),
        Err(JournalError::CorruptBlob { .. })
    ));
    assert!(matches!(
        db.append(&change(b"before", b"next")),
        Err(JournalError::CorruptBlob { .. })
    ));
    assert_eq!(table_counts(&db.connection()), before);
    let wrong = zstd::stream::encode_all(&b"wrong"[..], 0).expect("compress");
    db.connection()
        .execute(
            "UPDATE blobs SET zstd=?,bytes=? WHERE hash=?",
            params![wrong, wrong.len() as i64, first.files[0].before_hash],
        )
        .expect("corrupt address");
    assert!(
        matches!(db.get(first.seq), Err(JournalError::CorruptBlob { reason, .. }) if reason.contains("SHA-256"))
    );
    drop(db);
    let db = writer(&dir);
    assert!(
        db.quarantined_to.is_none(),
        "blob read errors preserve the original durable DB"
    );
}

#[test]
fn decompression_bomb_and_oversized_query_are_bounded() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let first = db.append(&change(b"before", b"after")).expect("first").op;
    let bomb = vec![b'x'; 64 * 1024 * 1024 + 1];
    let compressed = zstd::stream::encode_all(bomb.as_slice(), 0).expect("compress");
    drop(bomb);
    db.connection()
        .execute(
            "UPDATE blobs SET zstd=?,bytes=? WHERE hash=?",
            params![
                compressed,
                compressed.len() as i64,
                first.files[0].before_hash
            ],
        )
        .expect("large output");
    assert!(matches!(
        db.get(first.seq),
        Err(JournalError::ReadLimitExceeded(_))
    ));
    assert!(matches!(
        db.recent(&JournalLogQuery {
            limit: 1001,
            ..Default::default()
        }),
        Err(JournalError::InvalidInput(_))
    ));
}

#[test]
fn raw_list_refuses_truncation_and_recent_paginates_large_history() {
    let dir = tempfile::tempdir().expect("directory");
    let db = writer(&dir);
    db.connection().execute_batch("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1001) INSERT INTO ops(at,tool,args_hash,session_id) SELECT x,'edit','args','s' FROM n").expect("large history");
    assert!(matches!(
        db.list(0),
        Err(JournalError::ReadLimitExceeded(_))
    ));
    let first = db
        .recent(&JournalLogQuery {
            limit: 1000,
            ..Default::default()
        })
        .expect("first page");
    assert_eq!(first.len(), 1000);
    let next = db
        .recent(&JournalLogQuery {
            before_seq: Some(first.last().expect("last").op.seq),
            ..Default::default()
        })
        .expect("next page");
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].op.seq, 1);
}

#[test]
fn corrupt_durable_database_and_sidecars_are_quarantined_intact() {
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("journal.db");
    for suffix in ["", "-wal", "-shm", "-journal"] {
        std::fs::write(
            dir.path().join(format!("journal.db{suffix}")),
            format!("original {suffix}"),
        )
        .expect("corrupt files");
    }
    std::fs::write(
        dir.path().join("journal.db.quarantined-shm"),
        b"earlier recovery",
    )
    .expect("collision");
    let db = JournalDb::open(JournalDbOptions::new(&path)).expect("recovered journal");
    assert_eq!(
        db.quarantined_to.as_deref(),
        Some(
            dir.path()
                .canonicalize()
                .expect("canonical directory")
                .join("journal.db.quarantined.2")
                .as_path()
        )
    );
    assert_eq!(db.lifecycle_notes.len(), 1);
    assert!(db.lifecycle_notes[0].contains("quarantined intact"));
    for suffix in ["", "-wal", "-shm", "-journal"] {
        assert_eq!(
            std::fs::read(dir.path().join(format!("journal.db.quarantined.2{suffix}")))
                .expect("preserved bytes"),
            format!("original {suffix}").as_bytes()
        );
    }
    assert_eq!(
        std::fs::read(dir.path().join("journal.db.quarantined-shm")).expect("old recovery"),
        b"earlier recovery"
    );
    assert!(db.list(0).expect("fresh history").is_empty());
}

#[test]
fn future_and_nonempty_unknown_schema_fail_closed() {
    for (version, schema) in [
        (3, ""),
        (3, "CREATE TABLE future(x)"),
        (0, "CREATE TABLE old(x)"),
        (2, "CREATE TABLE impostor(x)"),
    ] {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("journal.db");
        let db = Connection::open(&path).expect("database");
        db.execute_batch(schema).expect("schema");
        db.pragma_update(None, "user_version", version)
            .expect("version");
        drop(db);
        let before = std::fs::read(&path).expect("before");
        assert!(
            matches!(
                JournalDb::open(JournalDbOptions::new(&path)),
                Err(JournalError::UnsupportedVersion(_))
            ),
            "version {version}"
        );
        assert_eq!(std::fs::read(&path).expect("after"), before);
        assert!(!dir.path().join("journal.db.quarantined").exists());
    }
}

#[test]
fn reader_never_quarantines_corruption_or_creates_missing_file() {
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("journal.db");
    assert!(JournalReader::open(&path).is_err());
    assert!(!path.exists());
    std::fs::write(&path, b"corrupt journal").expect("corrupt DB");
    assert!(JournalReader::open(&path).is_err());
    assert_eq!(std::fs::read(&path).expect("preserved"), b"corrupt journal");
    assert!(!dir.path().join("journal.db.quarantined").exists());
}

#[test]
fn orphaned_sidecars_are_preserved_when_main_file_is_missing() {
    for empty_placeholder in [false, true] {
        let dir = tempfile::tempdir().expect("directory");
        let path = dir.path().join("journal.db");
        if empty_placeholder {
            std::fs::write(&path, []).expect("empty placeholder");
        }
        let sidecar = dir.path().join("journal.db-wal");
        std::fs::write(&sidecar, b"durable orphan").expect("orphan");
        assert!(JournalDb::open(JournalDbOptions::new(&path)).is_err());
        assert_eq!(path.exists(), empty_placeholder);
        assert_eq!(
            std::fs::read(sidecar).expect("orphan preserved"),
            b"durable orphan"
        );
    }
}

#[test]
fn late_hydration_failure_rolls_back_eviction_of_older_blobs() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let older = db
        .append(&change(b"older before", b"older after"))
        .expect("older")
        .op;
    let reused = db
        .append(&change(b"reused before", b"reused after"))
        .expect("reused")
        .op;
    let hash = &reused.files[0].before_hash;
    db.connection()
        .execute("UPDATE blobs SET zstd=x'0001',bytes=2 WHERE hash=?", [hash])
        .expect("corrupt protected blob");
    let before = table_counts(&db.connection());
    let hashes = |db: &Connection| {
        db.prepare("SELECT hash FROM blobs ORDER BY hash")
            .expect("prepare")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("hashes")
    };
    let blobs_before = hashes(&db.connection());
    db.cap_bytes = 0;
    let mut input = change(b"reused before", b"new after");
    input.undo_of = Some(vec![older.seq]);
    assert!(matches!(
        db.append(&input),
        Err(JournalError::CorruptBlob { .. })
    ));
    assert_eq!(table_counts(&db.connection()), before);
    assert_eq!(hashes(&db.connection()), blobs_before);
    assert_eq!(db.get(older.seq).expect("older restored"), Some(older));
}

#[test]
fn corrupt_eviction_accounting_preserves_every_durable_row() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    db.append(&change(b"old before", b"old after"))
        .expect("older");
    db.connection()
        .execute("UPDATE blobs SET bytes=bytes+1", [])
        .expect("corrupt size counters");
    let before = table_counts(&db.connection());
    db.cap_bytes = 0;
    assert!(matches!(
        db.append(&change(b"fresh before", b"fresh after")),
        Err(JournalError::CorruptRecord(_))
    ));
    assert_eq!(table_counts(&db.connection()), before);
}

#[test]
fn advertised_zstd_window_is_bounded_independently_of_output_size() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let op = db.append(&change(b"before", b"after")).expect("op").op;
    // Valid Zstandard frame structure: no content size, a 2-GiB advertised
    // window, and an empty final raw block. Output-only limits miss this case.
    let frame = [0x28_u8, 0xb5, 0x2f, 0xfd, 0x00, 0xa8, 0x01, 0x00, 0x00];
    db.connection()
        .execute(
            "UPDATE blobs SET zstd=?,bytes=? WHERE hash=?",
            params![
                frame.as_slice(),
                frame.len() as i64,
                op.files[0].before_hash
            ],
        )
        .expect("large-window frame");
    assert!(
        matches!(db.get(op.seq), Err(JournalError::CorruptBlob { reason, .. }) if reason.contains("invalid zstd content"))
    );
}

#[test]
fn sqlite_row_allocation_is_bounded_before_metadata_hydration() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db = writer(&dir);
    let op = db.append(&change(b"before", b"after")).expect("op").op;
    let limit = rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH;
    db.connection()
        .set_limit(limit, 80 * 1024 * 1024)
        .expect("fixture limit");
    db.connection()
        .execute(
            "UPDATE ops SET tool=CAST(zeroblob(70*1024*1024) AS TEXT)",
            [],
        )
        .expect("oversized metadata fixture");
    db.connection()
        .set_limit(limit, 65 * 1024 * 1024)
        .expect("restore production limit");
    assert!(
        matches!(db.get(op.seq), Err(JournalError::Sqlite(rusqlite::Error::SqliteFailure(code, _))) if code.code == rusqlite::ErrorCode::TooBig)
    );
}

#[test]
fn busy_and_invalid_path_errors_never_quarantine() {
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("journal.db");
    let mut options = JournalDbOptions::new(&path);
    options.journal_mode = JournalMode::Truncate;
    let db = JournalDb::open(options.clone()).expect("journal");
    db.connection()
        .execute_batch("BEGIN EXCLUSIVE")
        .expect("exclusive lock");
    assert!(
        matches!(JournalDb::open(options), Err(JournalError::Sqlite(rusqlite::Error::SqliteFailure(code, _))) if code.code == rusqlite::ErrorCode::DatabaseBusy)
    );
    assert!(!dir.path().join("journal.db.quarantined").exists());
    db.connection().execute_batch("ROLLBACK").expect("unlock");
    let directory_path = dir.path().join("directory.db");
    std::fs::create_dir(&directory_path).expect("directory at DB path");
    assert!(matches!(
        JournalDb::open(JournalDbOptions::new(&directory_path)),
        Err(JournalError::InvalidInput(_))
    ));
    assert!(directory_path.is_dir());
}

#[cfg(unix)]
#[test]
fn linked_durable_files_are_refused_without_touching_targets() {
    let dir = tempfile::tempdir().expect("directory");
    let target = dir.path().join("target");
    std::fs::write(&target, b"unrelated durable bytes").expect("target");
    let path = dir.path().join("journal.db");
    std::os::unix::fs::symlink(&target, &path).expect("symlink");
    assert!(matches!(
        JournalDb::open(JournalDbOptions::new(&path)),
        Err(JournalError::InvalidInput(_))
    ));
    assert_eq!(
        std::fs::read(&target).expect("target"),
        b"unrelated durable bytes"
    );
}

#[test]
fn bundled_sqlite_runtime_carries_the_wal_reset_fix() {
    // SQLite 3.51.3 (2026-03-13) fixed the WAL-reset checkpoint race that
    // every release from 3.7.0 through 3.51.2 carried; the journal links the
    // bundled library, so the runtime it reports is the one every store uses.
    const WAL_RESET_FIX: i32 = 3_051_003;
    assert!(
        rusqlite::version_number() >= WAL_RESET_FIX,
        "linked SQLite {} predates the WAL-reset fix",
        rusqlite::version()
    );
    assert_eq!(
        rusqlite::version_number(),
        rusqlite::ffi::SQLITE_VERSION_NUMBER,
        "runtime library differs from the compiled bundled header"
    );
    let dir = tempfile::tempdir().expect("directory");
    let db = writer(&dir);
    let runtime: String = db
        .connection()
        .query_row("SELECT sqlite_version()", [], |row| row.get(0))
        .expect("runtime version");
    assert_eq!(runtime, rusqlite::version());
    assert_eq!(db.journal_mode().expect("mode"), "wal");
}

#[test]
fn read_only_reader_beside_the_sole_writer_reads_across_a_checkpoint() {
    // The journal's topology: one admitted writer that is also the only
    // checkpointer, and read-only readers that can neither write nor reset
    // the WAL. That is what keeps it outside the WAL-reset advisory.
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("journal.db");
    let mut db = writer(&dir);
    assert_eq!(db.journal_mode().expect("mode"), "wal");
    db.append(&change(b"first before", b"first after"))
        .expect("first append");
    let reader = JournalReader::open(&path).expect("reader beside the writer");
    assert_eq!(reader.list(0).expect("committed ops").len(), 1);
    db.append(&change(b"second before", b"second after"))
        .expect("append beside an open reader");
    assert_eq!(reader.list(0).expect("sees the new commit").len(), 2);

    let read_only = Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .expect("read-only connection");
    let refused = read_only
        .execute_batch("DELETE FROM ops")
        .expect_err("a read-only reader cannot write");
    assert_eq!(
        refused.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly)
    );

    let (busy, log, checkpointed): (i64, i64, i64) = db
        .connection()
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .expect("the writer checkpoints");
    assert_eq!(busy, 0, "no reader pins the WAL between reads");
    assert_eq!(log, checkpointed);
    assert_eq!(reader.list(0).expect("after checkpoint").len(), 2);
    assert_eq!(db.list(0).expect("writer view").len(), 2);
    let integrity: String = read_only
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity");
    assert_eq!(integrity, "ok");
}
