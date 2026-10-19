use super::*;
use crate::transaction::{
    BaselineSource, ExpectedState, FileDisposition, FormatMode, TransactionChange,
    TransactionStatus,
};
use crate::{
    config::ResolveOptions,
    file_content::FileContent,
    jail::WorkspaceKeyStore,
    journal::{JournalDbOptions, JournalWriteOutcome, UndoCandidateQuery},
    worker::WorkerOptions,
};

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
            root.path().join(".magistr/workspace/journal.db"),
        ))
        .unwrap();
        let key = WorkspaceKeyStore::new(root.path().join(".magistr/workspace"))
            .ensure()
            .unwrap();
        let seal = ConfirmationSeal::new(&key, 1);
        Self {
            root,
            policy,
            worker,
            journal,
            seal,
        }
    }
    fn service(&mut self) -> WorkspaceTransactions<'_> {
        WorkspaceTransactions::new(
            &self.policy,
            &self.worker,
            &mut self.journal,
            &self.seal,
            FormatterOptions::default(),
        )
        .unwrap()
    }
}
fn binary(bytes: &[u8]) -> FileContent {
    FileContent::decode(bytes.to_vec())
}
fn request(
    path: &str,
    expected: ExpectedState,
    content: Option<FileContent>,
) -> TransactionRequest {
    TransactionRequest {
        operation: "edit".into(),
        serialized_request: br#"{ "op": "edit" }"#.to_vec(),
        session_id: "session-a".into(),
        changes: vec![TransactionChange {
            path: path.into(),
            expected,
            content,
            baseline: None,
        }],
        undo_of: vec![],
        format: FormatMode::Disabled,
    }
}
fn pair(root: &std::path::Path) -> TransactionRequest {
    let old = binary(&[0, 1]);
    std::fs::write(root.join("a.bin"), old.as_bytes()).unwrap();
    std::fs::write(root.join("b.bin"), old.as_bytes()).unwrap();
    let mut request = request(
        "a.bin",
        ExpectedState::Hash(old.content_hash()),
        Some(binary(&[0, 2])),
    );
    let mut second = request.changes[0].clone();
    second.path = "b.bin".into();
    request.changes.push(second);
    request
}
#[tokio::test]
async fn binary_preview_is_lossless_and_application_has_durable_intent() {
    let mut h = Harness::new();
    let path = h.root.path().join("new.bin");
    let bytes = vec![0, 255, 1, 2, 3];
    let mut service = h.service();
    let prepared = service
        .prepare(request(
            "new.bin",
            ExpectedState::Missing,
            Some(FileContent::Binary(bytes.clone())),
        ))
        .await
        .unwrap();
    assert_eq!(prepared.preview()[0].before, None);
    assert_eq!(
        prepared.preview()[0].after.as_ref().unwrap().as_bytes(),
        bytes
    );
    assert!(!path.exists());
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Applied);
    let seq = result.journal_intent_seq.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(
        h.journal.write_outcome(seq).unwrap(),
        Some(JournalWriteOutcome::Applied)
    );
    assert_eq!(h.worker.status().await.pid, None);
}
#[tokio::test]
async fn stale_hash_and_absence_are_checked_before_preview_and_again_before_commit() {
    let mut h = Harness::new();
    let path = h.root.path().join("empty.bin");
    std::fs::write(&path, []).unwrap();
    let mut service = h.service();
    assert!(matches!(
        service
            .prepare(request(
                "empty.bin",
                ExpectedState::Missing,
                Some(binary(&[0, 2]))
            ))
            .await,
        Err(TransactionError::StaleAnchor(_))
    ));
    let prepared = service
        .prepare(request(
            "empty.bin",
            ExpectedState::Hash(binary(&[]).content_hash()),
            Some(binary(&[0, 2])),
        ))
        .await
        .unwrap();
    std::fs::write(&path, [0, 8]).unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert!(matches!(
        result.error,
        Some(TransactionError::StaleAnchor(_))
    ));
    assert_eq!(result.status, TransactionStatus::Failed);
    assert_eq!(result.journal_intent_seq, None);
    assert_eq!(std::fs::read(path).unwrap(), [0, 8]);
    assert!(h.journal.list(0).unwrap().is_empty());
}
#[tokio::test]
async fn private_final_plan_cannot_be_substituted_or_rebound_to_other_request_bytes() {
    let mut h = Harness::new();
    for tamper in 0..4 {
        let mut service = h.service();
        let mut prepared = service
            .prepare(request(
                "new.bin",
                ExpectedState::Missing,
                Some(binary(&[0, 2])),
            ))
            .await
            .unwrap();
        let token = prepared.confirmation_token().to_string();
        match tamper {
            0 => prepared.preview[0].after = Some(binary(&[0, 7])),
            1 => prepared.request.serialized_request.push(b' '),
            2 => prepared.request.session_id.push('x'),
            _ => prepared.preview[0].formatting.push_str("forged"),
        }
        let result = service.apply(prepared, &token).await;
        assert!(matches!(
            result.error,
            Some(TransactionError::Confirmation(_))
        ));
        assert_eq!(result.journal_intent_seq, None);
    }
    assert!(!h.root.path().join("new.bin").exists());
}
#[tokio::test]
async fn all_routes_recheck_protected_targets_and_duplicate_aliases() {
    let mut h = Harness::new();
    std::fs::create_dir(h.root.path().join(".git")).unwrap();
    for operation in ["edit", "files", "script", "undo"] {
        let mut input = request(".git/config", ExpectedState::Missing, Some(binary(&[0, 2])));
        input.operation = operation.into();
        assert!(matches!(
            h.service().prepare(input).await,
            Err(TransactionError::Policy(_))
        ));
    }
    let mut input = request("x.bin", ExpectedState::Missing, Some(binary(&[0, 2])));
    let mut duplicate = input.changes[0].clone();
    duplicate.path = "./x.bin".into();
    input.changes.push(duplicate);
    assert!(matches!(
        h.service().prepare(input).await,
        Err(TransactionError::InvalidInput(_))
    ));
}
#[tokio::test]
async fn parser_unavailable_and_fake_binary_text_cannot_bypass_shadow_verification() {
    let mut h = Harness::new();
    let result = h
        .service()
        .prepare(request(
            "broken.rs",
            ExpectedState::Missing,
            Some(FileContent::Binary(b"fn invalid( {".to_vec())),
        ))
        .await;
    assert!(matches!(result, Err(TransactionError::Worker(_))));
    assert!(!h.root.path().join("broken.rs").exists());
    assert!(h.journal.list(0).unwrap().is_empty());
}
#[tokio::test]
async fn journal_failure_precedes_any_swap() {
    let mut h = Harness::new();
    let path = h.root.path().join("new.bin");
    let control =
        rusqlite::Connection::open(h.root.path().join(".magistr/workspace/journal.db")).unwrap();
    control.execute_batch("CREATE TRIGGER fail_intent BEFORE INSERT ON ops BEGIN SELECT RAISE(ABORT, 'injected journal storage failure'); END;").unwrap();
    let mut service = h.service();
    let prepared = service
        .prepare(request(
            "new.bin",
            ExpectedState::Missing,
            Some(binary(&[0, 2])),
        ))
        .await
        .unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Failed);
    assert!(matches!(result.error, Some(TransactionError::Journal(_))));
    assert_eq!(result.journal_intent_seq, None);
    assert!(!path.exists());
    assert!(h.journal.list(0).unwrap().is_empty());
}
#[tokio::test]
async fn partial_swap_is_compensated_and_intent_is_not_an_undo_candidate() {
    let mut h = Harness::new();
    let input = pair(h.root.path());
    let mut service = h.service();
    service.faults.swap_at = Some(1);
    let prepared = service.prepare(input).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Failed);
    assert_eq!(result.files[0].disposition, FileDisposition::RolledBack);
    assert_eq!(result.files[1].disposition, FileDisposition::NotApplied);
    assert_eq!(std::fs::read(h.root.path().join("a.bin")).unwrap(), [0, 1]);
    assert_eq!(std::fs::read(h.root.path().join("b.bin")).unwrap(), [0, 1]);
    assert_eq!(
        h.journal
            .write_outcome(result.journal_intent_seq.unwrap())
            .unwrap(),
        Some(JournalWriteOutcome::RolledBack)
    );
    assert!(
        h.journal
            .undo_candidates(&UndoCandidateQuery::default())
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn failed_compensation_reports_observed_partial_application_and_indeterminate_intent() {
    let mut h = Harness::new();
    let input = pair(h.root.path());
    let mut service = h.service();
    service.faults.swap_at = Some(1);
    service.faults.compensation_at = Some(0);
    let prepared = service.prepare(input).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Indeterminate);
    assert_eq!(result.files[0].disposition, FileDisposition::Applied);
    assert_eq!(result.files[1].disposition, FileDisposition::NotApplied);
    assert_eq!(std::fs::read(h.root.path().join("a.bin")).unwrap(), [0, 2]);
    assert_eq!(std::fs::read(h.root.path().join("b.bin")).unwrap(), [0, 1]);
    assert_eq!(
        h.journal
            .write_outcome(result.journal_intent_seq.unwrap())
            .unwrap(),
        Some(JournalWriteOutcome::Indeterminate)
    );
    assert!(
        h.journal
            .undo_candidates(&UndoCandidateQuery::default())
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn failed_effect_after_rename_is_itself_compensated() {
    let mut h = Harness::new();
    let mut service = h.service();
    service.faults.after_swap_at = Some(0);
    let prepared = service
        .prepare(request(
            "new.bin",
            ExpectedState::Missing,
            Some(binary(&[0, 2])),
        ))
        .await
        .unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Failed);
    assert_eq!(result.files[0].disposition, FileDisposition::RolledBack);
    assert!(!h.root.path().join("new.bin").exists());
}
#[tokio::test]
async fn failed_outcome_persistence_reports_live_applied_files_and_pending_intent() {
    let mut h = Harness::new();
    let mut service = h.service();
    service.faults.resolve = true;
    let prepared = service
        .prepare(request(
            "new.bin",
            ExpectedState::Missing,
            Some(binary(&[0, 2])),
        ))
        .await
        .unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Indeterminate);
    assert_eq!(result.files[0].disposition, FileDisposition::Applied);
    assert_eq!(
        h.journal
            .write_outcome(result.journal_intent_seq.unwrap())
            .unwrap(),
        Some(JournalWriteOutcome::Pending)
    );
}
#[tokio::test]
async fn alternate_baseline_requires_fresh_admitted_source() {
    let mut h = Harness::new();
    let mut input = request("new.bin", ExpectedState::Missing, Some(binary(&[0, 2])));
    input.changes[0].baseline = Some(BaselineSource {
        path: "missing.rs".into(),
        expected_hash: binary(&[]).content_hash(),
    });
    assert!(matches!(
        h.service().prepare(input).await,
        Err(TransactionError::StaleAnchor(_))
    ));
}
#[tokio::test]
async fn disabled_undo_restores_exact_bytes_and_deletion_is_lossless_in_intent() {
    let mut h = Harness::new();
    let path = h.root.path().join("old.bin");
    std::fs::write(&path, [0, 255, 9]).unwrap();
    let mut service = h.service();
    let prepared = service
        .prepare(request(
            "old.bin",
            ExpectedState::Hash(binary(&[0, 255, 9]).content_hash()),
            None,
        ))
        .await
        .unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Applied);
    assert!(!path.exists());
    let op = h
        .journal
        .get(result.journal_intent_seq.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(op.files[0].before.as_ref().unwrap().as_bytes(), [0, 255, 9]);
    let mut undo = request(
        "old.bin",
        ExpectedState::Missing,
        op.files[0].before.clone(),
    );
    undo.operation = "undo".into();
    undo.undo_of = vec![op.seq];
    let mut service = h.service();
    let prepared = service.prepare(undo).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Applied);
    assert_eq!(std::fs::read(path).unwrap(), [0, 255, 9]);
    assert!(
        h.journal
            .undo_candidates(&UndoCandidateQuery::default())
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn empty_undo_bookkeeping_consumes_history_without_filesystem_effects() {
    let mut h = Harness::new();
    let original = h
        .journal
        .append(&crate::journal::JournalOpInit {
            tool: "edit".into(),
            args_hash: "original".into(),
            session_id: "session-a".into(),
            files: vec![],
            undo_of: None,
        })
        .unwrap()
        .op
        .seq;
    let mut input = request("ignored", ExpectedState::Missing, None);
    input.changes.clear();
    input.undo_of = vec![original];
    input.operation = "undo".into();
    let mut service = h.service();
    let prepared = service.prepare(input).await.unwrap();
    assert!(prepared.preview().is_empty());
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Applied);
    assert!(result.files.is_empty());
    assert!(result.journal_intent_seq.is_some());
    assert!(
        h.journal
            .undo_candidates(&UndoCandidateQuery::default())
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn unsupported_text_is_explicitly_unverified_and_identity_is_bounded_not_required() {
    let mut h = Harness::new();
    let mut input = request(
        "notes.md",
        ExpectedState::Missing,
        Some(binary(b"some text")),
    );
    input.format = FormatMode::Enabled;
    let prepared = h.service().prepare(input.clone()).await.unwrap();
    assert_eq!(
        prepared.preview()[0].verification,
        crate::transaction::VerificationStatus::Unsupported
    );
    assert!(prepared.preview()[0].formatting.contains("unsupported"));
    // The empty session is the retained anonymous identity; the seal binds
    // it like any other, so the token differs from the named session's.
    let named_token = prepared.confirmation_token().to_owned();
    input.session_id.clear();
    let anonymous = h.service().prepare(input.clone()).await.unwrap();
    assert_ne!(anonymous.confirmation_token(), named_token);
    input.session_id = "s".repeat(4097);
    assert!(matches!(
        h.service().prepare(input).await,
        Err(TransactionError::InvalidInput(_))
    ));
    assert_eq!(h.worker.status().await.pid, None);
}
#[tokio::test]
async fn undo_formatting_and_resource_bounds_are_refused_without_effects() {
    let mut h = Harness::new();
    let mut input = request("x.bin", ExpectedState::Missing, Some(binary(&[0, 2])));
    input.format = FormatMode::Enabled;
    input.undo_of = vec![1];
    assert!(matches!(
        h.service().prepare(input).await,
        Err(TransactionError::InvalidInput(_))
    ));
    let mut input = request("x.bin", ExpectedState::Missing, Some(binary(&[0, 2])));
    input.changes = vec![input.changes[0].clone(); 257];
    assert!(matches!(
        h.service().prepare(input).await,
        Err(TransactionError::InvalidInput(_))
    ));
    let mut input = request("x.bin", ExpectedState::Missing, Some(binary(&[0, 2])));
    input.serialized_request = vec![0; 1024 * 1024 + 1];
    assert!(matches!(
        h.service().prepare(input).await,
        Err(TransactionError::InvalidInput(_))
    ));
    assert!(h.journal.list(0).unwrap().is_empty());
}

#[tokio::test]
async fn observed_swap_and_compensation_errors_still_report_verified_rollback() {
    let mut h = Harness::new();
    let path = h.root.path().join("existing.bin");
    let before = binary(&[0, 1]);
    std::fs::write(&path, before.as_bytes()).unwrap();
    let mut service = h.service();
    service.faults.after_swap_at = Some(0);
    service.faults.after_compensation_at = Some(0);
    let prepared = service
        .prepare(request(
            "existing.bin",
            ExpectedState::Hash(before.content_hash()),
            Some(binary(&[0, 2])),
        ))
        .await
        .unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(std::fs::read(path).unwrap(), before.as_bytes());
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.contains("post-rename compensation failure"))
    );
    assert_eq!(result.status, TransactionStatus::Failed);
    assert_eq!(result.files[0].disposition, FileDisposition::RolledBack);
    assert_eq!(
        h.journal
            .write_outcome(result.journal_intent_seq.unwrap())
            .unwrap(),
        Some(JournalWriteOutcome::RolledBack),
    );
}

fn nested_batch(root: &std::path::Path, failing: usize) -> TransactionRequest {
    // `deep/` exists before the batch; `deep/new/` and `fresh/` do not.
    std::fs::create_dir(root.join("deep")).unwrap();
    let mut request = request(
        "deep/new/inner/a.bin",
        ExpectedState::Missing,
        Some(binary(&[0, 2])),
    );
    let mut second = request.changes[0].clone();
    second.path = "fresh/b.bin".into();
    request.changes.push(second);
    let mut third = request.changes[0].clone();
    third.path = format!("failing-{failing}.bin");
    request.changes.push(third);
    request
}
#[tokio::test]
async fn rolled_back_nested_creation_removes_only_the_directories_the_batch_created() {
    let mut h = Harness::new();
    let input = nested_batch(h.root.path(), 2);
    let mut service = h.service();
    service.faults.swap_at = Some(2);
    let prepared = service.prepare(input).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Failed);
    assert_eq!(result.files[0].disposition, FileDisposition::RolledBack);
    assert_eq!(result.files[1].disposition, FileDisposition::RolledBack);
    assert_eq!(result.files[2].disposition, FileDisposition::NotApplied);
    assert!(
        h.root.path().join("deep").is_dir(),
        "preexisting parent kept"
    );
    assert!(
        !h.root.path().join("deep/new").exists(),
        "batch-created chain removed"
    );
    assert!(
        !h.root.path().join("fresh").exists(),
        "batch-created parent removed"
    );
    assert!(
        !result.notes.iter().any(|note| note.contains("directory")),
        "{:?}",
        result.notes
    );
}
#[tokio::test]
async fn compensation_keeps_a_created_directory_that_gained_entries_and_names_it() {
    let mut h = Harness::new();
    let input = nested_batch(h.root.path(), 2);
    let foreign = h.root.path().join("fresh/foreign.txt");
    let mut service = h.service();
    service.faults.swap_at = Some(2);
    service.faults.before_directory_cleanup = Some(Box::new(move || {
        std::fs::write(&foreign, b"someone else's").unwrap();
    }));
    let prepared = service.prepare(input).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Failed);
    assert_eq!(result.files[1].disposition, FileDisposition::RolledBack);
    assert!(!h.root.path().join("deep/new").exists());
    assert!(!h.root.path().join("fresh/b.bin").exists());
    assert_eq!(
        std::fs::read(h.root.path().join("fresh/foreign.txt")).unwrap(),
        b"someone else's"
    );
    assert!(
        result
            .notes
            .iter()
            .any(|note| note.starts_with("fresh: compensation left a directory it created")),
        "{:?}",
        result.notes
    );
}
#[tokio::test]
async fn failed_file_compensation_leaves_its_created_directories_and_names_them() {
    let mut h = Harness::new();
    let input = nested_batch(h.root.path(), 2);
    let mut service = h.service();
    service.faults.swap_at = Some(2);
    service.faults.compensation_at = Some(0);
    let prepared = service.prepare(input).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Indeterminate);
    assert_eq!(result.files[0].disposition, FileDisposition::Applied);
    assert_eq!(
        std::fs::read(h.root.path().join("deep/new/inner/a.bin")).unwrap(),
        [0, 2]
    );
    assert!(!h.root.path().join("fresh").exists());
    let left: Vec<_> = result
        .notes
        .iter()
        .filter(|note| note.contains("compensation left a directory it created"))
        .collect();
    assert_eq!(left.len(), 1, "{:?}", result.notes);
    assert!(left[0].starts_with("deep/new/inner: "), "{}", left[0]);
}
#[tokio::test]
async fn a_directory_is_claimed_once_by_the_change_that_created_it() {
    let mut h = Harness::new();
    let mut input = request(
        "shared/a.bin",
        ExpectedState::Missing,
        Some(binary(&[0, 2])),
    );
    let mut second = input.changes[0].clone();
    second.path = "shared/b.bin".into();
    input.changes.push(second);
    let mut third = input.changes[0].clone();
    third.path = "c.bin".into();
    input.changes.push(third);
    let mut service = h.service();
    service.faults.swap_at = Some(2);
    let prepared = service.prepare(input).await.unwrap();
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Failed);
    assert!(!h.root.path().join("shared").exists());
    assert!(result.notes.iter().all(|note| !note.contains("directory")));
}

#[cfg(unix)]
#[tokio::test]
async fn literal_backslash_filename_survives_preview_application_and_journal() {
    let mut h = Harness::new();
    let relative = r"a\b.unknown";
    let path = h.root.path().join(relative);
    let bytes = binary(&[0, 255, 9]);
    let mut service = h.service();
    let prepared = service
        .prepare(request(
            relative,
            ExpectedState::Missing,
            Some(bytes.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(prepared.preview()[0].path, relative);
    let token = prepared.confirmation_token().to_string();
    let result = service.apply(prepared, &token).await;
    assert_eq!(result.status, TransactionStatus::Applied);
    assert_eq!(std::fs::read(path).unwrap(), bytes.as_bytes());
    let operation = h
        .journal
        .get(result.journal_intent_seq.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(operation.files[0].path, relative);
    assert_eq!(operation.files[0].after.as_ref(), Some(&bytes));
    assert_eq!(h.worker.status().await.pid, None);
}
