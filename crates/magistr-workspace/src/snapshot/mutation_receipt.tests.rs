use super::*;
use crate::{
    file_content::FileContent,
    journal::{JournalDbOptions, JournalFileChange, JournalOpInit},
};

fn op(path: &str, after: &[u8]) -> JournalOpInit {
    JournalOpInit {
        tool: "edit".into(),
        args_hash: "bound-args".into(),
        session_id: "session-a".into(),
        files: vec![JournalFileChange {
            kind: JournalFileKind::Write,
            path: path.into(),
            before: FileContent::decode(vec![0, 255]),
            after: FileContent::decode(after.to_vec()),
        }],
        undo_of: None,
    }
}

fn journal() -> (tempfile::TempDir, JournalDb) {
    let dir = tempfile::tempdir().expect("directory");
    let db = JournalDb::open(JournalDbOptions::new(dir.path().join("journal.db"))).expect("writer");
    (dir, db)
}

#[test]
fn only_a_resolved_applied_intent_publishes_a_revision() {
    let (_dir, mut db) = journal();
    let applied = db
        .append_intent(&op("a.bin", &[255, 0]))
        .expect("intent")
        .op
        .seq;
    db.resolve_intent(applied, JournalWriteOutcome::Applied)
        .expect("applied");
    let rolled = db.append_intent(&op("b.bin", &[1])).expect("intent").op.seq;
    db.resolve_intent(rolled, JournalWriteOutcome::RolledBack)
        .expect("rolled back");
    let broken = db.append_intent(&op("c.bin", &[2])).expect("intent").op.seq;
    db.resolve_intent(broken, JournalWriteOutcome::Indeterminate)
        .expect("indeterminate");
    let pending = db.append_intent(&op("d.bin", &[3])).expect("intent").op.seq;

    let receipt = MutationReceipt::from_journal(&db, applied, Some(4))
        .expect("read")
        .expect("exists");
    assert_eq!(receipt.status, MutationStatus::Applied);
    assert_eq!(receipt.generation, Some(4));
    assert_eq!(receipt.tool, "edit");
    assert_eq!(receipt.session_id, "session-a");
    assert_eq!(receipt.files.len(), 1);
    assert_eq!(receipt.files[0].kind, "write");
    assert_eq!(
        receipt.files[0].after_hash,
        FileContent::decode(vec![255, 0]).content_hash()
    );
    assert!(receipt.notes.is_empty());
    let revision = receipt.revision.clone().expect("applied revision");
    assert_eq!(
        MutationReceipt::from_journal(&db, applied, Some(4))
            .expect("read")
            .expect("exists")
            .revision,
        Some(revision)
    );

    for (seq, status) in [
        (rolled, MutationStatus::Failed),
        (broken, MutationStatus::Indeterminate),
        (pending, MutationStatus::Indeterminate),
    ] {
        let receipt = MutationReceipt::from_journal(&db, seq, None)
            .expect("read")
            .expect("exists");
        assert_eq!(receipt.status, status, "seq {seq}");
        assert_eq!(receipt.revision, None, "seq {seq} has no revision");
        assert_eq!(receipt.notes.len(), 1, "seq {seq} explains itself");
    }
    assert!(
        MutationReceipt::from_journal(&db, 99, None)
            .expect("read")
            .is_none()
    );
}

#[test]
fn an_interrupted_owner_leaves_an_indeterminate_receipt_readable_without_a_writer() {
    let (dir, mut db) = journal();
    let seq = db
        .append_intent(&op("file.bin", &[7, 0, 7]))
        .expect("intent")
        .op
        .seq;
    drop(db);
    let reader = JournalReader::open(dir.path().join("journal.db")).expect("reader");
    let receipt = MutationReceipt::from_reader(&reader, seq, Some(2))
        .expect("read")
        .expect("exists");
    assert_eq!(receipt.status, MutationStatus::Indeterminate);
    assert_eq!(receipt.revision, None);
    assert!(receipt.notes[0].contains("never resolved"));
    // The bytes behind the receipt's hashes are intact.
    let op = reader.get(seq).expect("op").expect("exists");
    assert_eq!(
        op.files[0].after.as_ref().expect("bytes").as_bytes(),
        &[7, 0, 7]
    );
    assert_eq!(op.files[0].after_hash, receipt.files[0].after_hash);
    let json = serde_json::to_string(&receipt).expect("serialize");
    let parsed: MutationReceipt = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, receipt);
}

#[test]
fn legacy_history_without_native_intent_is_never_certified() {
    let (_dir, mut db) = journal();
    let seq = db
        .append(&op("old.bin", &[1, 2]))
        .expect("legacy op")
        .op
        .seq;
    assert_eq!(db.write_outcome(seq).expect("outcome"), None);
    let receipt = MutationReceipt::from_journal(&db, seq, None)
        .expect("read")
        .expect("exists");
    assert_eq!(receipt.status, MutationStatus::Indeterminate);
    assert_eq!(receipt.revision, None);
    assert!(receipt.notes[0].contains("legacy"));
}
