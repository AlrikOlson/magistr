use super::*;
use crate::journal::{JournalFileChange, JournalFileKind, JournalReader, JournalWriteOutcome};

fn source(root: &std::path::Path) -> JournalDb {
    let path = root.join("old.db");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(include_str!("../../fixtures/upgrades/workbench-v3.sql"))
        .unwrap();
    drop(db);
    JournalDb::open(JournalDbOptions::new(path)).unwrap()
}

fn op(session: &str, before: &str, after: &str) -> JournalOpInit {
    JournalOpInit {
        tool: "edit".into(),
        args_hash: "args".into(),
        session_id: session.into(),
        undo_of: None,
        files: vec![JournalFileChange {
            kind: JournalFileKind::Write,
            path: "file.txt".into(),
            before: FileContent::Text(before.into()),
            after: FileContent::Text(after.into()),
        }],
    }
}

#[test]
fn import_preserves_current_ids_wal_rows_snapshots_undo_links_and_pending_outcomes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let mut old = source(root);
    let first = old.append(&op("old", "a", "b")).unwrap().op;
    let mut undo = op("old", "b", "a");
    undo.undo_of = Some(vec![first.seq]);
    let undo = old.append_intent(&undo).unwrap().op;
    old.resolve_intent(undo.seq, JournalWriteOutcome::Applied)
        .unwrap();
    let pending = old.append_intent(&op("old", "a", "pending")).unwrap().op;
    let latest = old.append_intent(&op("old", "a", "c")).unwrap().op;
    old.resolve_intent(latest.seq, JournalWriteOutcome::Applied)
        .unwrap();
    assert!(
        root.join("old.db-wal").exists(),
        "exercise committed rows still in WAL"
    );
    let source_before = old.list(0).unwrap();
    let path = root.join("current.db");
    let mut current = JournalDb::open(JournalDbOptions::new(&path)).unwrap();
    let a = current.append(&op("current", "c", "d")).unwrap().op;
    let b = current.append(&op("current", "d", "e")).unwrap().op;
    current.import_workbench(&root.join("old.db")).unwrap();
    assert_eq!(current.get(a.seq).unwrap(), Some(a.clone()));
    assert_eq!(current.get(b.seq).unwrap(), Some(b.clone()));
    assert_eq!(
        old.list(0).unwrap(),
        source_before,
        "import never writes older data"
    );
    let imported = current.get(b.seq + first.seq).unwrap().unwrap();
    assert_eq!(
        imported.files, first.files,
        "exact content snapshots survive"
    );
    let imported_undo = current.get(b.seq + undo.seq).unwrap().unwrap();
    assert_eq!(imported_undo.undo_of, Some(vec![b.seq + first.seq]));
    assert_eq!(
        current.write_outcome(b.seq + pending.seq).unwrap(),
        Some(JournalWriteOutcome::Pending)
    );
    let recent = current.recent(&JournalLogQuery::default()).unwrap();
    assert_eq!(
        recent.iter().map(|r| r.op.seq).collect::<Vec<_>>(),
        vec![2, 1, 6, 5, 4, 3]
    );
    assert_eq!(recent.last().unwrap().undone_by, Some(4));
    let candidates = current
        .undo_candidates(&UndoCandidateQuery::default())
        .unwrap();
    assert_eq!(
        candidates.iter().map(|op| op.seq).collect::<Vec<_>>(),
        vec![2, 1, 6]
    );
    let page = current
        .recent(&JournalLogQuery {
            before_seq: Some(1),
            limit: 2,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        page.iter().map(|r| r.op.seq).collect::<Vec<_>>(),
        vec![6, 5]
    );
    let page = current
        .recent(&JournalLogQuery {
            before_seq: Some(5),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        page.iter().map(|r| r.op.seq).collect::<Vec<_>>(),
        vec![4, 3]
    );
    current.import_workbench(&root.join("old.db")).unwrap();
    assert_eq!(current.list(0).unwrap().len(), 6);
    let mut undo_imported = op("current", "c", "a");
    undo_imported.undo_of = Some(vec![6]);
    let next = current.append_intent(&undo_imported).unwrap().op;
    assert_eq!(next.seq, 7);
    current
        .resolve_intent(next.seq, JournalWriteOutcome::Applied)
        .unwrap();
    drop(current);
    let reader = JournalReader::open(&path).unwrap();
    assert_eq!(
        reader
            .recent(&JournalLogQuery::default())
            .unwrap()
            .first()
            .unwrap()
            .op
            .seq,
        7
    );
    let reopened = JournalDb::open(JournalDbOptions::new(&path)).unwrap();
    assert!(reopened.workbench_imported().unwrap());
    assert_eq!(reopened.get(7).unwrap().unwrap().undo_of, Some(vec![6]));
}

#[test]
fn failed_import_rolls_back_all_rows_schema_and_completion_then_retries_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut old = source(dir.path());
    old.append(&op("old", "a", "b")).unwrap();
    old.append(&op("old", "b", "c")).unwrap();
    let current = JournalDb::open(JournalDbOptions::new(dir.path().join("current.db"))).unwrap();
    current.connection().execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON ops WHEN NEW.seq=2 BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert!(
        current
            .import_workbench(&dir.path().join("old.db"))
            .is_err()
    );
    assert!(!current.workbench_imported().unwrap());
    assert!(current.list(0).unwrap().is_empty());
    let version: i64 = current
        .connection()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2, "schema changes roll back too");
    current
        .connection()
        .execute_batch("DROP TRIGGER fail_import")
        .unwrap();
    current
        .import_workbench(&dir.path().join("old.db"))
        .unwrap();
    assert_eq!(current.list(0).unwrap().len(), 2);
    current
        .import_workbench(&dir.path().join("old.db"))
        .unwrap();
    assert_eq!(current.list(0).unwrap().len(), 2);
}

#[test]
fn empty_destination_retains_old_ids_and_unknown_or_damaged_sources_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut old = source(dir.path());
    old.connection()
        .execute_batch("INSERT INTO sqlite_sequence(name,seq) VALUES ('ops',40)")
        .unwrap();
    let original = old.append(&op("old", "before", "after")).unwrap().op;
    assert_eq!(original.seq, 41);
    let current = JournalDb::open(JournalDbOptions::new(dir.path().join("current.db"))).unwrap();
    old.connection()
        .execute_batch("PRAGMA user_version=99")
        .unwrap();
    assert!(
        current
            .import_workbench(&dir.path().join("old.db"))
            .is_err()
    );
    assert!(current.list(0).unwrap().is_empty());
    old.connection()
        .execute_batch("PRAGMA user_version=3")
        .unwrap();
    current
        .import_workbench(&dir.path().join("old.db"))
        .unwrap();
    assert_eq!(current.get(41).unwrap(), Some(original));
    let other = JournalDb::open(JournalDbOptions::new(dir.path().join("other.db"))).unwrap();
    old.connection()
        .execute_batch("UPDATE blobs SET zstd=X'00',bytes=1")
        .unwrap();
    assert!(other.import_workbench(&dir.path().join("old.db")).is_err());
    assert!(other.list(0).unwrap().is_empty());
    assert!(!other.workbench_imported().unwrap());
}
