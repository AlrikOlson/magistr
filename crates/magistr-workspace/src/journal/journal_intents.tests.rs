use super::*;
use crate::journal::{JournalFileChange, JournalFileKind, JournalReader, JournalWriteOutcome};

fn op() -> JournalOpInit {
    JournalOpInit {
        tool: "edit".into(),
        args_hash: "bound-args".into(),
        session_id: "session".into(),
        files: vec![JournalFileChange {
            kind: JournalFileKind::Write,
            path: "file.bin".into(),
            before: FileContent::decode(vec![0, 255]),
            after: FileContent::decode(vec![255, 0]),
        }],
        undo_of: None,
    }
}

#[test]
fn pending_intent_is_visible_but_never_undoable_after_restart() {
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("journal.db");
    let mut db = JournalDb::open(JournalDbOptions::new(&path)).expect("writer");
    let seq = db.append_intent(&op()).expect("intent").op.seq;
    assert_eq!(
        db.write_outcome(seq).expect("outcome"),
        Some(JournalWriteOutcome::Pending)
    );
    assert!(
        db.undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates")
            .is_empty()
    );
    assert!(!db.recent(&JournalLogQuery::default()).expect("view")[0].undoable);
    drop(db);
    let reader = JournalReader::open(&path).expect("reader");
    assert_eq!(
        reader.write_outcome(seq).expect("read-only outcome"),
        Some(JournalWriteOutcome::Pending)
    );
    assert!(
        reader
            .undo_candidates(&UndoCandidateQuery::default())
            .expect("reader candidates")
            .is_empty()
    );
    assert_eq!(
        reader.get(seq).expect("op").expect("exists").files[0]
            .after
            .as_ref()
            .expect("bytes")
            .as_bytes(),
        &[255, 0]
    );
    drop(reader);
    let db = JournalDb::open(JournalDbOptions::new(&path)).expect("reopen");
    assert_eq!(
        db.write_outcome(seq).expect("outcome"),
        Some(JournalWriteOutcome::Pending)
    );
}

#[test]
fn only_applied_resolution_consumes_original_undo_history() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db =
        JournalDb::open(JournalDbOptions::new(dir.path().join("journal.db"))).expect("writer");
    let original = db.append(&op()).expect("legacy").op.seq;
    for status in [
        JournalWriteOutcome::RolledBack,
        JournalWriteOutcome::Indeterminate,
        JournalWriteOutcome::Applied,
    ] {
        let mut undo = op();
        undo.undo_of = Some(vec![original]);
        let seq = db.append_intent(&undo).expect("undo intent").op.seq;
        assert_eq!(
            db.get(seq).expect("intent").expect("op").undo_of,
            Some(vec![original])
        );
        assert!(
            db.undo_candidates(&UndoCandidateQuery::default())
                .expect("before resolution")
                .iter()
                .any(|op| op.seq == original)
        );
        db.resolve_intent(seq, status).expect("resolution");
        assert_eq!(db.write_outcome(seq).expect("outcome"), Some(status));
        assert_eq!(
            db.undo_candidates(&UndoCandidateQuery::default())
                .expect("candidates")
                .iter()
                .any(|op| op.seq == original),
            status != JournalWriteOutcome::Applied
        );
        assert!(
            db.resolve_intent(seq, JournalWriteOutcome::Pending)
                .is_err()
        );
        assert!(
            db.resolve_intent(
                seq,
                if status == JournalWriteOutcome::Applied {
                    JournalWriteOutcome::RolledBack
                } else {
                    JournalWriteOutcome::Applied
                }
            )
            .is_err()
        );
    }
}

#[test]
fn resolution_failure_rolls_back_outcome_and_undo_links() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db =
        JournalDb::open(JournalDbOptions::new(dir.path().join("journal.db"))).expect("writer");
    let original = db.append(&op()).expect("original").op.seq;
    let mut undo = op();
    undo.undo_of = Some(vec![original]);
    let seq = db.append_intent(&undo).expect("intent").op.seq;
    db.db.execute_batch("CREATE TRIGGER fail_resolution BEFORE UPDATE ON native_write_intents BEGIN SELECT RAISE(ABORT,'fault'); END").expect("inject");
    assert!(
        db.resolve_intent(seq, JournalWriteOutcome::Applied)
            .is_err()
    );
    assert_eq!(
        db.write_outcome(seq).expect("outcome"),
        Some(JournalWriteOutcome::Pending)
    );
    assert!(
        db.undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates")
            .iter()
            .any(|op| op.seq == original)
    );
}

#[test]
fn unresolved_intent_bytes_survive_later_retention_until_resolved() {
    let dir = tempfile::tempdir().expect("directory");
    let mut options = JournalDbOptions::new(dir.path().join("journal.db"));
    options.cap_bytes = 0;
    let mut db = JournalDb::open(options).expect("writer");
    let pending = db.append_intent(&op()).expect("pending").op.seq;
    let uncertain = db.append_intent(&op()).expect("uncertain").op.seq;
    db.resolve_intent(uncertain, JournalWriteOutcome::Indeterminate)
        .expect("resolve");
    let mut other = op();
    other.files[0].before = FileContent::decode(vec![0, 10]);
    other.files[0].after = FileContent::decode(vec![0, 20]);
    db.append(&other).expect("later append");
    for seq in [pending, uncertain] {
        let history = db.get(seq).expect("get").expect("history");
        assert_eq!(
            history.files[0]
                .before
                .as_ref()
                .expect("before preserved")
                .as_bytes(),
            &[0, 255]
        );
        assert_eq!(
            history.files[0]
                .after
                .as_ref()
                .expect("after preserved")
                .as_bytes(),
            &[255, 0]
        );
    }
}

#[test]
fn failed_first_intent_rolls_back_migration_and_preserves_history() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db =
        JournalDb::open(JournalDbOptions::new(dir.path().join("journal.db"))).expect("writer");
    let original = db.append(&op()).expect("original").op;
    let mut invalid = op();
    invalid.undo_of = Some(vec![9999]);
    assert!(db.append_intent(&invalid).is_err());
    assert_eq!(db.list(0).expect("unchanged"), vec![original]);
    assert_eq!(
        db.db
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .expect("version"),
        2
    );
    assert_eq!(
        db.db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name LIKE 'native_%'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .expect("schema"),
        0
    );
}

#[test]
fn native_schema_cannot_be_reinterpreted_by_downgrading_its_header() {
    let dir = tempfile::tempdir().expect("directory");
    let path = dir.path().join("journal.db");
    let mut db = JournalDb::open(JournalDbOptions::new(&path)).expect("writer");
    db.append_intent(&op()).expect("intent");
    db.db
        .pragma_update(None, "user_version", 2)
        .expect("simulate downgrade");
    drop(db);
    let before = std::fs::read(&path).expect("before");
    assert!(matches!(
        JournalReader::open(&path),
        Err(JournalError::UnsupportedVersion(_))
    ));
    assert!(matches!(
        JournalDb::open(JournalDbOptions::new(&path)),
        Err(JournalError::UnsupportedVersion(_))
    ));
    assert_eq!(std::fs::read(&path).expect("unchanged"), before);
}

#[test]
fn first_intent_migrates_atomically_without_rewriting_retained_blobs() {
    let dir = tempfile::tempdir().expect("directory");
    let mut db =
        JournalDb::open(JournalDbOptions::new(dir.path().join("journal.db"))).expect("writer");
    let prior = db.append(&op()).expect("prior").op;
    let compressed: Vec<u8> = db
        .db
        .query_row(
            "SELECT zstd FROM blobs WHERE hash=?",
            [&prior.files[0].before_hash],
            |r| r.get(0),
        )
        .expect("blob");
    let seq = db.append_intent(&op()).expect("intent").op.seq;
    assert_eq!(
        db.db
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .expect("version"),
        3
    );
    assert_eq!(
        db.db
            .query_row(
                "SELECT zstd FROM blobs WHERE hash=?",
                [&prior.files[0].before_hash],
                |r| r.get::<_, Vec<u8>>(0)
            )
            .expect("blob"),
        compressed
    );
    db.resolve_intent(seq, JournalWriteOutcome::Applied)
        .expect("resolve");
    assert!(
        db.undo_candidates(&UndoCandidateQuery::default())
            .expect("candidates")
            .iter()
            .any(|op| op.seq == seq)
    );
}
