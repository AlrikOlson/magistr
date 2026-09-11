use super::*;
use crate::journal::{JournalFileKind, JournalFileRecord};

fn bytes(value: &[u8]) -> FileContent {
    FileContent::decode(value.to_vec())
}

fn change(path: &str, kind: JournalFileKind, before: &[u8], after: &[u8]) -> JournalFileRecord {
    JournalFileRecord {
        kind,
        path: path.into(),
        before_hash: bytes(before).content_hash(),
        after_hash: bytes(after).content_hash(),
        before: Some(bytes(before)),
        after: Some(bytes(after)),
    }
}

fn op(seq: i64, files: Vec<JournalFileRecord>) -> JournalOp {
    JournalOp {
        seq,
        at: 1700000000000 + seq,
        tool: "edit".into(),
        args_hash: bytes(b"args").content_hash(),
        session_id: "caller".into(),
        undo_of: None,
        files,
    }
}

fn snapshots(items: &[(&str, Option<&[u8]>)]) -> BTreeMap<String, Option<FileContent>> {
    items
        .iter()
        .map(|(path, value)| ((*path).into(), value.map(bytes)))
        .collect()
}

#[test]
fn chained_binary_rewind_restores_original_and_anchors_observed_bytes() {
    let original = [0, 255, 13, 10, 128];
    let middle = [0, 254, 1];
    let latest = [0, 253, 2];
    let candidates = [
        op(
            2,
            vec![change("asset", JournalFileKind::Write, &middle, &latest)],
        ),
        op(
            1,
            vec![change("asset", JournalFileKind::Write, &original, &middle)],
        ),
    ];
    let current = snapshots(&[("asset", Some(&latest))]);
    let plan = UndoPlan::build(&candidates, &current, UndoConflictPolicy::Surface).unwrap();
    assert_eq!(plan.undo_of, [2, 1]);
    assert_eq!(plan.writes.len(), 1);
    assert_eq!(plan.writes[0].content, Some(bytes(&original)));
    assert_eq!(plan.writes[0].expected_hash, bytes(&latest).content_hash());
    assert!(plan.writes[0].expected_exists);
    assert_eq!(
        current["asset"],
        Some(bytes(&latest)),
        "planning must not mutate snapshots"
    );
}

#[test]
fn creation_and_deletion_distinguish_missing_from_empty() {
    let candidates = [op(
        1,
        vec![
            change("created", JournalFileKind::Create, b"", b""),
            change("deleted", JournalFileKind::Delete, b"", b""),
        ],
    )];
    let plan = UndoPlan::build(
        &candidates,
        &snapshots(&[("created", Some(b"")), ("deleted", None)]),
        UndoConflictPolicy::Surface,
    )
    .unwrap();
    assert_eq!(plan.writes.len(), 2);
    assert_eq!(plan.writes[0].path, "created");
    assert_eq!(plan.writes[0].content, None);
    assert!(plan.writes[0].expected_exists);
    assert_eq!(plan.writes[1].content, Some(bytes(b"")));
    assert!(!plan.writes[1].expected_exists);
    assert_eq!(plan.writes[0].expected_hash, plan.writes[1].expected_hash);
}

#[test]
fn surface_blocks_whole_batch_and_keep_or_force_are_explicit() {
    let candidates = [op(
        1,
        vec![
            change("z", JournalFileKind::Write, b"old z", b"new z"),
            change("a", JournalFileKind::Write, b"old a", b"new a"),
        ],
    )];
    let current = snapshots(&[("z", Some(b"new z")), ("a", Some(b"outside"))]);
    let surface = UndoPlan::build(&candidates, &current, UndoConflictPolicy::Surface).unwrap();
    assert!(surface.writes.is_empty());
    assert!(surface.undo_of.is_empty());
    assert_eq!(surface.conflicts.len(), 1);
    assert_eq!(
        surface.conflicts[0].current_hash,
        Some(bytes(b"outside").content_hash())
    );
    let keep = UndoPlan::build(&candidates, &current, UndoConflictPolicy::KeepCurrent).unwrap();
    assert_eq!(keep.undo_of, [1]);
    assert_eq!(keep.writes.len(), 1);
    assert_eq!(keep.kept[0].path, "a");
    let force = UndoPlan::build(&candidates, &current, UndoConflictPolicy::Force).unwrap();
    assert_eq!(
        force
            .writes
            .iter()
            .map(|w| w.path.as_str())
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(force.writes[1].content, Some(bytes(b"old a")));
    assert_eq!(
        force.writes[1].expected_hash,
        bytes(b"outside").content_hash()
    );
    assert!(!force.notes.is_empty());
}

#[test]
fn missing_blob_skips_entire_operation_without_consuming_it() {
    let mut unavailable = change("lost", JournalFileKind::Write, b"old", b"new");
    unavailable.before = None;
    let candidate = op(
        1,
        vec![
            unavailable,
            change("available", JournalFileKind::Write, b"a", b"b"),
        ],
    );
    let plan = UndoPlan::build(&[candidate], &BTreeMap::new(), UndoConflictPolicy::Force).unwrap();
    assert!(plan.writes.is_empty());
    assert!(plan.undo_of.is_empty());
    assert_eq!(plan.evicted.len(), 2);
    assert!(!plan.notes.is_empty());
}

#[test]
fn irrelevant_empty_blob_eviction_does_not_block_create_or_delete() {
    let mut create = change("create", JournalFileKind::Create, b"", b"new");
    create.before = None;
    let mut delete = change("delete", JournalFileKind::Delete, b"old", b"");
    delete.after = None;
    let plan = UndoPlan::build(
        &[op(1, vec![create, delete])],
        &snapshots(&[("create", Some(b"new")), ("delete", None)]),
        UndoConflictPolicy::Surface,
    )
    .unwrap();
    assert_eq!(plan.undo_of, [1]);
    assert_eq!(plan.writes.len(), 2);
    assert!(plan.evicted.is_empty());
}

#[test]
fn no_change_and_kept_operations_still_have_proposed_bookkeeping() {
    let same = op(1, vec![change("same", JournalFileKind::Write, b"x", b"x")]);
    let plan = UndoPlan::build(
        &[same],
        &snapshots(&[("same", Some(b"x"))]),
        UndoConflictPolicy::Surface,
    )
    .unwrap();
    assert!(plan.writes.is_empty());
    assert_eq!(plan.undo_of, [1]);
    let plan = UndoPlan::build(
        &[op(2, vec![])],
        &BTreeMap::new(),
        UndoConflictPolicy::Surface,
    )
    .unwrap();
    assert_eq!(plan.undo_of, [2]);
}

#[test]
fn all_kept_proposes_only_bookkeeping_and_absence_conflicts_keep_typed_hashes() {
    let candidates = [op(
        1,
        vec![change("empty", JournalFileKind::Create, b"", b"")],
    )];
    let actual = snapshots(&[("empty", None)]);
    let surface = UndoPlan::build(&candidates, &actual, UndoConflictPolicy::Surface).unwrap();
    assert_eq!(surface.conflicts.len(), 1);
    assert_eq!(
        surface.conflicts[0].expected_hash,
        Some(bytes(b"").content_hash())
    );
    assert_eq!(surface.conflicts[0].current_hash, None);
    assert_eq!(surface.conflicts[0].restore_hash, None);
    let keep = UndoPlan::build(&candidates, &actual, UndoConflictPolicy::KeepCurrent).unwrap();
    assert!(keep.writes.is_empty());
    assert_eq!(keep.undo_of, [1]);
    assert_eq!(keep.kept.len(), 1);
    let force = UndoPlan::build(&candidates, &actual, UndoConflictPolicy::Force).unwrap();
    assert!(
        force.writes.is_empty(),
        "forced restoration already equals the missing path"
    );
    assert_eq!(force.undo_of, [1]);
}

#[test]
fn absent_snapshot_is_not_fabricated_as_missing_file() {
    let candidate = op(
        1,
        vec![change("file", JournalFileKind::Write, b"old", b"new")],
    );
    assert_eq!(
        UndoPlan::build(&[candidate], &BTreeMap::new(), UndoConflictPolicy::Force),
        Err(UndoPlanError::MissingSnapshot("file".into()))
    );
}

#[test]
fn invalid_order_duplicates_or_undo_operations_are_refused() {
    let mut undo = op(3, vec![]);
    undo.undo_of = Some(vec![1]);
    for candidates in [
        vec![op(1, vec![]), op(2, vec![])],
        vec![op(2, vec![]), op(2, vec![])],
        vec![undo],
        vec![op(0, vec![])],
        vec![op(
            1,
            vec![
                change("same", JournalFileKind::Write, b"a", b"b"),
                change("same", JournalFileKind::Write, b"b", b"c"),
            ],
        )],
        (1..=101).rev().map(|seq| op(seq, vec![])).collect(),
    ] {
        assert_eq!(
            UndoPlan::build(&candidates, &BTreeMap::new(), UndoConflictPolicy::Surface),
            Err(UndoPlanError::InvalidCandidates)
        );
    }
}
