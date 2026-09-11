use super::*;
use crate::{
    commands::{EditOp, FilesOp, UndoScope, WriteAction},
    config::ResolveOptions,
    contracts::{ErrorKind, WorkspaceResponse},
    file_content::FileContent,
    jail::WorkspaceKeyStore,
    journal::JournalDbOptions,
    undo::UndoConflictPolicy,
    worker::WorkerOptions,
};
use std::fs;

/// Worker-free harness: `.txt` and binary targets take the unsupported /
/// binary verification lanes, so verbs, gating, anchors, trash, undo and
/// journaling are exercised without a parser child. Source languages need
/// the real worker and live in the magistr-cli integration suite.
struct Harness {
    root: tempfile::TempDir,
    policy: WorkspacePolicy,
    worker: WorkerSupervisor,
    journal: JournalDb,
    seal: ConfirmationSeal,
}

impl Harness {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(root.path())).unwrap();
        let worker =
            WorkerSupervisor::new("/unavailable-worker".into(), WorkerOptions::default()).unwrap();
        let journal = JournalDb::open(JournalDbOptions::new(
            root.path().join(".workbench/journal.db"),
        ))
        .unwrap();
        let key = WorkspaceKeyStore::new(root.path().join(".workbench"))
            .ensure()
            .unwrap();
        Self {
            root,
            policy,
            worker,
            journal,
            seal: ConfirmationSeal::new(&key),
        }
    }

    fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let file = self.root.path().join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content).unwrap();
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        fs::read(self.root.path().join(path)).ok()
    }

    fn commands(&mut self, session: &str) -> WorkspaceCommands<'_> {
        WorkspaceCommands::new(
            &self.policy,
            &self.worker,
            &mut self.journal,
            &self.seal,
            None,
            FormatterOptions::default(),
            session,
        )
        .unwrap()
    }
}

fn ok<R: std::fmt::Debug + Clone>(response: WorkspaceResponse<R>) -> (Vec<R>, Vec<String>) {
    match response {
        WorkspaceResponse::Success { results, notes, .. } => {
            (results.rows().to_vec(), notes.unwrap_or_default())
        }
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("expected success, got {error} {notes:?}")
        }
    }
}

fn refused<R: std::fmt::Debug>(response: WorkspaceResponse<R>) -> (ErrorKind, Vec<String>) {
    match response {
        WorkspaceResponse::Failure { error, notes, .. } => (error.kind, notes.unwrap_or_default()),
        WorkspaceResponse::Success { results, notes, .. } => {
            panic!("expected refusal, got {:?} {notes:?}", results.rows())
        }
    }
}

fn replace_text(file: &str, old: &str, new: &str) -> EditInput {
    let mut input = EditInput::new(EditOp::ReplaceText);
    input.file = Some(file.into());
    input.old = Some(old.into());
    input.new = Some(new.into());
    input
}

fn files(op: FilesOp, from: Option<&str>, to: Option<&str>) -> FilesInput {
    let mut input = FilesInput::new(op);
    input.from = from.map(str::to_owned);
    input.to = to.map(str::to_owned);
    input
}

#[tokio::test]
async fn replace_text_applies_the_addressed_occurrence_through_the_pipeline_and_journals_it() {
    let mut h = Harness::new();
    h.write("notes/a.txt", "one 1\ntwo 1\n");
    let mut input = replace_text("notes/a.txt", "1", "9");
    input.occurrence = Some(2);
    let (rows, notes) = ok(h.commands("s").edit(&input).await);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].applied);
    assert_eq!(rows[0].action, WriteAction::Modified);
    assert!(
        rows[0].diff.contains("-two 1\n+two 9\n"),
        "{}",
        rows[0].diff
    );
    assert!(rows[0].journal_seq.is_some());
    assert!(notes[0].contains("replaced occurrence 2 of 2"));
    assert_eq!(h.read("notes/a.txt").unwrap(), b"one 1\ntwo 9\n");
    let all = ok(h
        .commands("s")
        .edit(&replace_text("notes/a.txt", "9", "3"))
        .await)
    .0;
    assert_eq!(
        all[0].content_hash.as_deref(),
        Some(&*FileContent::Text("one 1\ntwo 3\n".into()).content_hash())
    );
    let ops = h.journal.list(0).unwrap();
    assert_eq!(ops.len(), 2);
    assert!(
        ops.iter()
            .all(|op| op.tool == "edit" && op.session_id == "s")
    );
}

#[tokio::test]
async fn stale_anchors_and_missing_files_answer_the_retained_taxonomy_with_fresh_hashes() {
    let mut h = Harness::new();
    h.write("a.txt", "alpha\n");
    let fresh = FileContent::Text("alpha\n".into()).content_hash();
    let mut stale = replace_text("a.txt", "alpha", "beta");
    stale.expected_hash = Some("0".repeat(64));
    let (kind, _) = refused(h.commands("s").edit(&stale).await);
    assert_eq!(
        kind,
        ErrorKind::StaleAnchor {
            content_hash: fresh.clone()
        }
    );
    let (kind, _) = refused(
        h.commands("s")
            .edit(&replace_text("a.txt", "missing anchor", "x"))
            .await,
    );
    assert_eq!(
        kind,
        ErrorKind::StaleAnchor {
            content_hash: fresh.clone()
        }
    );
    let mut out_of_range = replace_text("a.txt", "alpha", "x");
    out_of_range.occurrence = Some(99);
    assert!(matches!(
        refused(h.commands("s").edit(&out_of_range).await).0,
        ErrorKind::StaleAnchor { .. }
    ));
    let (kind, _) = refused(
        h.commands("s")
            .edit(&replace_text("no/such.txt", "a", "b"))
            .await,
    );
    assert_eq!(kind, ErrorKind::FileNotFound);
    let (kind, _) = refused(h.commands("s").edit(&replace_text("a.txt", "", "b")).await);
    assert_eq!(kind, ErrorKind::PatternInvalid);
    let mut symbolic = EditInput::new(EditOp::ReplaceBody);
    symbolic.file = Some("a.txt".into());
    symbolic.symbol = Some("alpha".into());
    symbolic.content = Some("x".into());
    let (kind, notes) = refused(h.commands("s").edit(&symbolic).await);
    assert_eq!(kind, ErrorKind::IndexUnavailable);
    assert!(notes[0].contains("no index exists"));
    assert_eq!(h.read("a.txt").unwrap(), b"alpha\n");
    assert!(h.journal.list(0).unwrap().is_empty());
}

#[tokio::test]
async fn large_edits_are_confirm_gated_with_the_prepared_token_and_dry_runs_touch_nothing() {
    let mut h = Harness::new();
    let big: String = (0..300).map(|i| format!("line {i}\n")).collect();
    h.write("notes/big.txt", &big);
    let input = replace_text("notes/big.txt", "line", "row");
    let (kind, notes) = refused(h.commands("s").edit(&input).await);
    let ErrorKind::ConfirmRequired { confirm_token } = kind else {
        panic!("gated");
    };
    assert!(confirm_token.starts_with("mgw1."));
    assert!(notes.iter().any(|n| n.contains("+row 0")), "{notes:?}");
    assert_eq!(h.read("notes/big.txt").unwrap(), big.as_bytes());

    let mut preview = input.clone();
    preview.dry_run = true;
    let (rows, _) = ok(h.commands("s").edit(&preview).await);
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].applied);
    assert_eq!(h.read("notes/big.txt").unwrap(), big.as_bytes());
    assert!(h.journal.list(0).unwrap().is_empty());

    let mut forged = input.clone();
    forged.confirm = Some("wb1.forged".into());
    assert_eq!(
        refused(h.commands("s").edit(&forged).await).0,
        ErrorKind::ConfirmMismatch
    );
    let mut other_session = input.clone();
    other_session.confirm = Some(confirm_token.clone());
    assert_eq!(
        refused(h.commands("other").edit(&other_session).await).0,
        ErrorKind::ConfirmMismatch
    );

    let mut changed_args = input.clone();
    changed_args.new = Some("column".into());
    changed_args.confirm = Some(confirm_token.clone());
    assert_eq!(
        refused(h.commands("s").edit(&changed_args).await).0,
        ErrorKind::ConfirmMismatch
    );

    let mut confirmed = input.clone();
    confirmed.confirm = Some(confirm_token);
    let (rows, _) = ok(h.commands("s").edit(&confirmed).await);
    assert!(rows[0].applied);
    assert!(h.read("notes/big.txt").unwrap().starts_with(b"row 0\n"));
}

#[tokio::test]
async fn files_verbs_journal_one_op_each_and_delete_lands_in_the_trash() {
    let mut h = Harness::new();
    h.write("src/a.txt", "content a\n");
    ok(h.commands("suite")
        .files(&files(FilesOp::Create, None, Some("src/new.txt")))
        .await);
    ok(h.commands("suite")
        .files(&files(FilesOp::Mkdir, None, Some("src/dir")))
        .await);
    ok(h.commands("suite")
        .files(&files(FilesOp::Copy, Some("src/a.txt"), Some("src/b.txt")))
        .await);
    let (rows, notes) = ok(h
        .commands("suite")
        .files(&files(FilesOp::Delete, Some("src/b.txt"), None))
        .await);
    assert_eq!(rows[0].action, WriteAction::Deleted);
    assert!(notes[0].contains("recoverable with undo"));
    let mut moved = files(FilesOp::Move, Some("src/a.txt"), Some("deep/moved.txt"));
    moved.auto_apply = true;
    let (rows, notes) = ok(h.commands("suite").files(&moved).await);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.applied));
    assert!(notes[0].contains("go imports name packages, not files, and are not rewritten"));
    assert!(!notes[0].to_lowercase().contains("rust"));
    assert_eq!(h.read("deep/moved.txt").unwrap(), b"content a\n");
    assert!(h.read("src/a.txt").is_none());
    assert!(h.read("src/dir/.gitkeep").is_some());

    let ops = h.journal.list(0).unwrap();
    assert_eq!(ops.len(), 5);
    assert!(
        ops.iter()
            .all(|op| op.tool == "files" && op.session_id == "suite")
    );
    let kinds: Vec<Vec<String>> = ops
        .iter()
        .map(|op| {
            let mut kinds: Vec<String> = op
                .files
                .iter()
                .map(|f| format!("{:?}:{}", f.kind, f.path).to_lowercase())
                .collect();
            kinds.sort();
            kinds
        })
        .collect();
    assert_eq!(kinds[0], vec!["create:src/new.txt"]);
    assert_eq!(kinds[1], vec!["create:src/dir/.gitkeep"]);
    assert_eq!(kinds[2], vec!["create:src/b.txt"]);
    assert_eq!(kinds[3], vec!["delete:src/b.txt"]);
    assert_eq!(kinds[4], vec!["create:deep/moved.txt", "delete:src/a.txt"]);

    let trash: Vec<_> = fs::read_dir(h.root.path().join(".workbench/trash"))
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect();
    assert_eq!(trash.len(), 2, "delete and move both trash the source");
    let trashed_b = trash
        .iter()
        .find(|entry| entry.file_name().to_string_lossy().ends_with("-b.txt"))
        .expect("trashed copy keeps the original name");
    assert_eq!(fs::read(trashed_b.path()).unwrap(), b"content a\n");
}

#[tokio::test]
async fn files_verb_semantics_noops_gating_and_protected_paths() {
    let mut h = Harness::new();
    h.write("src/a.txt", "a\n");
    h.write("src/b.txt", "b\n");
    h.write("src/same.txt", "a\n");
    let (rows, notes) = ok(h
        .commands("s")
        .files(&files(FilesOp::Create, None, Some("src/a.txt")))
        .await);
    assert!(rows.is_empty());
    assert!(notes.iter().any(|n| n.contains("left untouched")));
    ok(h.commands("s")
        .files(&files(FilesOp::Mkdir, None, Some("src/dir")))
        .await);
    let (rows, _) = ok(h
        .commands("s")
        .files(&files(FilesOp::Mkdir, None, Some("src/dir")))
        .await);
    assert!(rows.is_empty());
    assert_eq!(h.journal.list(0).unwrap().len(), 1);
    let (kind, _) = refused(
        h.commands("s")
            .files(&files(FilesOp::Copy, Some("src/a.txt"), Some("src/b.txt")))
            .await,
    );
    assert!(matches!(kind, ErrorKind::ConfirmRequired { .. }));
    assert_eq!(h.read("src/b.txt").unwrap(), b"b\n");
    let (rows, _) = ok(h
        .commands("s")
        .files(&files(
            FilesOp::Copy,
            Some("src/a.txt"),
            Some("src/same.txt"),
        ))
        .await);
    assert!(rows.is_empty());
    let (kind, _) = refused(
        h.commands("s")
            .files(&files(
                FilesOp::Move,
                Some("src/a.txt"),
                Some("./src/a.txt"),
            ))
            .await,
    );
    assert_eq!(kind, ErrorKind::PatternInvalid);
    let (kind, _) = refused(
        h.commands("s")
            .files(&files(FilesOp::Delete, Some("src/ghost.txt"), None))
            .await,
    );
    assert_eq!(kind, ErrorKind::FileNotFound);
    let (kind, _) = refused(
        h.commands("s")
            .files(&files(FilesOp::Delete, Some(".workbench/journal.db"), None))
            .await,
    );
    assert_eq!(kind, ErrorKind::ProtectedPath);
    let (kind, _) = refused(
        h.commands("s")
            .files(&files(FilesOp::Move, Some("src/a.txt"), None))
            .await,
    );
    assert_eq!(kind, ErrorKind::PatternInvalid);
    let mut stale = files(FilesOp::Move, Some("src/a.txt"), Some("src/c.txt"));
    stale.expected_hash = Some("0".repeat(64));
    stale.auto_apply = true;
    let (kind, _) = refused(h.commands("s").files(&stale).await);
    assert_eq!(
        kind,
        ErrorKind::StaleAnchor {
            content_hash: FileContent::Text("a\n".into()).content_hash()
        }
    );
    let big: String = (0..250).map(|i| format!("x{i}\n")).collect();
    h.write("src/big.txt", &big);
    let (kind, _) = refused(
        h.commands("s")
            .files(&files(FilesOp::Delete, Some("src/big.txt"), None))
            .await,
    );
    assert!(matches!(kind, ErrorKind::ConfirmRequired { .. }));
    assert!(h.read("src/big.txt").is_some());
    let mut preview = files(FilesOp::Move, Some("src/a.txt"), Some("src/c.txt"));
    preview.dry_run = true;
    let (rows, _) = ok(h.commands("s").files(&preview).await);
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| !r.applied));
    assert!(h.read("src/a.txt").is_some());
}

fn undo(scope: UndoScope, policy: UndoConflictPolicy) -> UndoInput {
    UndoInput {
        scope,
        on_conflict: policy,
        ..UndoInput::default()
    }
}

#[tokio::test]
async fn undo_is_session_scoped_and_reverts_edits_creates_and_deletes_exactly() {
    let mut h = Harness::new();
    h.write("src/a.txt", "const a = 1\n");
    ok(h.commands("alice")
        .edit(&replace_text("src/a.txt", "1", "2"))
        .await);
    let (rows, notes) = ok(h.commands("bob").undo(&UndoInput::default()).await);
    assert!(rows.is_empty());
    assert!(
        notes
            .iter()
            .any(|n| n.contains("nothing to undo in session scope"))
    );
    assert_eq!(h.read("src/a.txt").unwrap(), b"const a = 2\n");
    let (rows, notes) = ok(h.commands("alice").undo(&UndoInput::default()).await);
    assert!(matches!(rows[0], UndoRow::Write(ref r) if r.applied));
    assert!(
        notes
            .iter()
            .any(|n| n.contains("reverted op(s) 1 in session scope"))
    );
    assert_eq!(h.read("src/a.txt").unwrap(), b"const a = 1\n");
    let log = h.journal.list(0).unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[1].tool, "undo");
    assert_eq!(log[1].undo_of, Some(vec![1]));
    assert_eq!(log[1].session_id, "alice");
    let (rows, notes) = ok(h.commands("alice").undo(&UndoInput::default()).await);
    assert!(rows.is_empty());
    assert!(notes.iter().any(|n| n.contains("nothing to undo")));

    ok(h.commands("s")
        .files(&files(FilesOp::Create, None, Some("src/made.txt")))
        .await);
    ok(h.commands("s").undo(&UndoInput::default()).await);
    assert!(h.read("src/made.txt").is_none());
    h.write("src/gone.bin", [0, 255, 7]);
    ok(h.commands("s")
        .files(&files(FilesOp::Delete, Some("src/gone.bin"), None))
        .await);
    assert!(h.read("src/gone.bin").is_none());
    ok(h.commands("s").undo(&UndoInput::default()).await);
    assert_eq!(h.read("src/gone.bin").unwrap(), vec![0, 255, 7]);
    // Workspace scope is the explicit, deliberate cross-session rewind.
    ok(h.commands("s")
        .edit(&replace_text("src/a.txt", "1", "5"))
        .await);
    ok(h.commands("bob")
        .undo(&undo(UndoScope::Workspace, UndoConflictPolicy::Surface))
        .await);
    assert_eq!(h.read("src/a.txt").unwrap(), b"const a = 1\n");
}

#[tokio::test]
async fn undo_steps_and_named_ops_thread_content_and_guard_sessions() {
    let mut h = Harness::new();
    h.write("src/a.txt", "v1\n");
    ok(h.commands("s")
        .edit(&replace_text("src/a.txt", "v1", "v2"))
        .await);
    ok(h.commands("s")
        .edit(&replace_text("src/a.txt", "v2", "v3"))
        .await);
    let two = UndoInput {
        steps: 2,
        ..UndoInput::default()
    };
    let (_, notes) = ok(h.commands("s").undo(&two).await);
    assert!(notes.iter().any(|n| n.contains("reverted op(s)")));
    assert_eq!(h.read("src/a.txt").unwrap(), b"v1\n");
    let log = h.journal.list(0).unwrap();
    assert_eq!(log.last().unwrap().undo_of, Some(vec![1, 2]));

    h.write("src/b.txt", "b1\n");
    ok(h.commands("alice")
        .edit(&replace_text("src/a.txt", "v1", "a2"))
        .await);
    ok(h.commands("alice")
        .edit(&replace_text("src/b.txt", "b1", "b2"))
        .await);
    let named = UndoInput {
        ops: Some(vec![4]),
        ..UndoInput::default()
    };
    let (_, notes) = ok(h.commands("alice").undo(&named).await);
    assert_eq!(h.read("src/a.txt").unwrap(), b"v1\n");
    assert_eq!(h.read("src/b.txt").unwrap(), b"b2\n");
    assert!(notes.iter().any(|n| n.contains("reverted op(s) 4")));
    let again = UndoInput {
        ops: Some(vec![4]),
        steps: 5,
        ..UndoInput::default()
    };
    let (rows, notes) = ok(h.commands("alice").undo(&again).await);
    assert!(rows.is_empty());
    assert!(notes.iter().any(|n| n.contains("not undoable")));
    assert!(notes.iter().any(|n| n.contains("steps is ignored")));
    let guarded = UndoInput {
        ops: Some(vec![5]),
        ..UndoInput::default()
    };
    let (rows, _) = ok(h.commands("bob").undo(&guarded).await);
    assert!(rows.is_empty());
    assert_eq!(h.read("src/b.txt").unwrap(), b"b2\n");
    let (kind, _) = refused(
        h.commands("bob")
            .undo(&UndoInput {
                ops: Some(vec![]),
                ..UndoInput::default()
            })
            .await,
    );
    assert_eq!(kind, ErrorKind::PatternInvalid);
    let (kind, _) = refused(
        h.commands("bob")
            .undo(&UndoInput {
                ops: Some(vec![0]),
                ..UndoInput::default()
            })
            .await,
    );
    assert_eq!(kind, ErrorKind::PatternInvalid);
}

#[tokio::test]
async fn undo_after_an_external_edit_surfaces_the_three_way_choice_then_keeps_or_forces() {
    let mut h = Harness::new();
    h.write("src/a.txt", "v1\n");
    ok(h.commands("s")
        .edit(&replace_text("src/a.txt", "v1", "v2"))
        .await);
    h.write("src/a.txt", "v3\n");
    let (rows, notes) = ok(h.commands("s").undo(&UndoInput::default()).await);
    let UndoRow::Conflict {
        path,
        expected_hash,
        current_hash,
        restore_hash,
        ..
    } = &rows[0]
    else {
        panic!("conflict row: {rows:?}");
    };
    assert_eq!(path, "src/a.txt");
    assert_eq!(
        *expected_hash,
        FileContent::Text("v2\n".into()).content_hash()
    );
    assert_eq!(
        *current_hash,
        FileContent::Text("v3\n".into()).content_hash()
    );
    assert_eq!(
        *restore_hash,
        FileContent::Text("v1\n".into()).content_hash()
    );
    assert_eq!(h.read("src/a.txt").unwrap(), b"v3\n");
    assert_eq!(h.journal.list(0).unwrap().len(), 1);
    let joined = notes.join(" ");
    assert!(
        joined.contains("keepCurrent")
            && joined.contains("force")
            && joined.contains("merge by hand")
    );

    ok(h.commands("s")
        .undo(&undo(UndoScope::Session, UndoConflictPolicy::Force))
        .await);
    assert_eq!(h.read("src/a.txt").unwrap(), b"v1\n");
    let (_, notes) = ok(h.commands("s").undo(&UndoInput::default()).await);
    assert!(notes.iter().any(|n| n.contains("nothing to undo")));

    h.write("src/b.txt", "b1\n");
    ok(h.commands("s")
        .edit(&replace_text("src/b.txt", "b1", "b2"))
        .await);
    h.write("src/b.txt", "outside\n");
    let (rows, _) = ok(h
        .commands("s")
        .undo(&undo(UndoScope::Session, UndoConflictPolicy::KeepCurrent))
        .await);
    assert!(matches!(rows[0], UndoRow::Kept { .. }));
    assert_eq!(h.read("src/b.txt").unwrap(), b"outside\n");
    let (_, notes) = ok(h.commands("s").undo(&UndoInput::default()).await);
    assert!(
        notes.iter().any(|n| n.contains("nothing to undo")),
        "the kept choice consumed the op"
    );
}

#[tokio::test]
async fn an_oversized_aggregated_diff_is_cut_honestly_in_the_confirm_answer() {
    let mut h = Harness::new();
    let huge: String = (0..600).map(|i| format!("line {i}\n")).collect();
    h.write("notes/huge.txt", &huge);
    let (kind, notes) = refused(
        h.commands("s")
            .edit(&replace_text("notes/huge.txt", "line", "row"))
            .await,
    );
    assert!(matches!(kind, ErrorKind::ConfirmRequired { .. }));
    assert!(
        notes
            .iter()
            .any(|n| n.contains("aggregated diff truncated at 8000 chars")),
        "{notes:?}"
    );
    assert!(
        notes.iter().all(|n| !n.contains("+row 0")),
        "a diff over the cap is cut, not partially shown"
    );
}
