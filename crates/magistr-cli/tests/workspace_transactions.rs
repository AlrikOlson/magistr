//! Native transaction acceptance through the real parser child and formatters.
use std::{fs, path::PathBuf};

use magistr_workspace::{
    config::ResolveOptions,
    confirmation::ConfirmationSeal,
    file_content::FileContent,
    jail::WorkspaceKeyStore,
    journal::{
        JournalDb, JournalDbOptions, JournalReader, JournalWriteOutcome, UndoCandidateQuery,
    },
    transaction::{
        ExpectedState, FileDisposition, FormatMode, FormatterOptions, TransactionChange,
        TransactionError, TransactionRequest, TransactionStatus, WorkspaceTransactions,
    },
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    policy: WorkspacePolicy,
    worker: WorkerSupervisor,
    journal: JournalDb,
    seal: ConfirmationSeal,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("transaction fixture");
        let root = directory.path().join("workspace");
        fs::create_dir_all(root.join(".git")).expect("repository marker");
        let key = WorkspaceKeyStore::new(root.join(".workbench"))
            .ensure()
            .expect("isolated workspace key");
        let journal = JournalDb::open(JournalDbOptions::new(root.join(".workbench/journal.db")))
            .expect("sole test journal writer");
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(&root)).expect("policy");
        let worker = WorkerSupervisor::new(
            PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
            WorkerOptions::default(),
        )
        .expect("native parser worker");
        Self {
            _directory: directory,
            root,
            policy,
            worker,
            journal,
            seal: ConfirmationSeal::new(&key),
        }
    }

    fn write(&self, path: &str, bytes: impl AsRef<[u8]>) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().expect("fixture parent")).expect("fixture directory");
        fs::write(file, bytes).expect("fixture source");
    }
}

fn change(path: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> TransactionChange {
    TransactionChange {
        path: path.into(),
        expected: before.map_or(ExpectedState::Missing, |bytes| {
            ExpectedState::Hash(FileContent::decode(bytes.to_vec()).content_hash())
        }),
        content: after.map(|bytes| FileContent::decode(bytes.to_vec())),
        baseline: None,
    }
}

fn request(changes: Vec<TransactionChange>, format: FormatMode) -> TransactionRequest {
    TransactionRequest {
        operation: "edit".into(),
        serialized_request: br#"{"test":"native transaction acceptance"}"#.to_vec(),
        session_id: "transaction-integration".into(),
        changes,
        undo_of: Vec::new(),
        format,
    }
}

#[tokio::test]
async fn real_rustfmt_previews_final_bytes_then_confirms_and_journals_those_bytes() {
    let mut fixture = Fixture::new();
    fixture.write(
        "Cargo.toml",
        "[package]\nname = \"transaction-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    fixture.write("rustfmt.toml", "tab_spaces = 2\n");
    let before = b"pub fn value() -> usize { 1 }\n";
    let candidate = b"pub async fn value()->usize{let value=2;value}\n";
    let expected = b"pub async fn value() -> usize {\n  let value = 2;\n  value\n}\n";
    fixture.write("src/lib.rs", before);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            fixture.root.join("src/lib.rs"),
            fs::Permissions::from_mode(0o750),
        )
        .expect("executable source mode");
    }

    let prepared = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("native transactions");
        transactions
            .prepare(request(
                vec![change("src/lib.rs", Some(before), Some(candidate))],
                FormatMode::Enabled,
            ))
            .await
            .expect("real parse and rustfmt preview")
    };
    assert_eq!(
        prepared.preview()[0]
            .after
            .as_ref()
            .expect("after")
            .as_bytes(),
        expected,
        "preview must include rustfmt output and directory config"
    );
    assert!(prepared.preview()[0].diagnostics.is_empty());
    assert_eq!(
        fs::read(fixture.root.join("src/lib.rs")).expect("source"),
        before
    );
    assert!(
        fixture
            .journal
            .list(0)
            .expect("journal after preview")
            .is_empty()
    );
    assert_ne!(fixture.worker.status().await.pid, Some(std::process::id()));
    assert!(
        fixture.worker.status().await.pid.is_some(),
        "real parser child ran"
    );

    let token = prepared.confirmation_token().to_owned();
    let outcome = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("same exclusive transaction composition");
        transactions.apply(prepared, &token).await
    };
    assert_eq!(outcome.status, TransactionStatus::Applied, "{outcome:?}");
    assert!(outcome.error.is_none());
    assert_eq!(
        fs::read(fixture.root.join("src/lib.rs")).expect("applied"),
        expected
    );
    let seq = outcome.journal_intent_seq.expect("journal intent");
    let op = fixture
        .journal
        .get(seq)
        .expect("durable operation")
        .expect("operation retained");
    assert_eq!(op.session_id, "transaction-integration");
    assert_eq!(op.files.len(), 1);
    assert_eq!(
        op.files[0].before.as_ref().expect("before blob").as_bytes(),
        before
    );
    assert_eq!(
        op.files[0].after.as_ref().expect("after blob").as_bytes(),
        expected
    );
    assert_eq!(
        op.files[0].after_hash,
        FileContent::decode(expected.to_vec()).content_hash()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(fixture.root.join("src/lib.rs"))
                .expect("source metadata")
                .permissions()
                .mode()
                & 0o777,
            0o750,
            "atomic replacement preserves the original mode"
        );
    }
    let journal_path = fixture.root.join(".workbench/journal.db");
    let reader = JournalReader::open(&journal_path).expect("read-only outcome view");
    assert_eq!(
        reader.write_outcome(seq).expect("read-only outcome"),
        Some(JournalWriteOutcome::Applied)
    );
    assert_eq!(reader.get(seq).expect("read-only operation"), Some(op));
    drop(reader);
    drop(fixture.journal);
    let restarted = JournalDb::open(JournalDbOptions::new(journal_path))
        .expect("reopen after sole writer shuts down");
    assert!(restarted.quarantined_to.is_none(), "v3 remains readable");
    assert_eq!(
        restarted.write_outcome(seq).expect("restart outcome"),
        Some(JournalWriteOutcome::Applied)
    );
    let candidates = restarted
        .undo_candidates(&UndoCandidateQuery {
            session_id: Some("transaction-integration".into()),
            ..Default::default()
        })
        .expect("applied intent is eligible after restart");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].seq, seq);
    fixture.worker.shutdown().await.expect("reap parser child");
}

#[tokio::test]
async fn real_parser_rejects_the_whole_shadow_batch_before_journal_or_swap() {
    let mut fixture = Fixture::new();
    let before = b"pub fn first() {}\n";
    fixture.write("first.rs", before);
    let result = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("transactions");
        transactions
            .prepare(request(
                vec![
                    change("first.rs", Some(before), Some(b"pub fn changed() {}\n")),
                    change("second.rs", None, Some(b"pub fn broken( {\n")),
                ],
                FormatMode::Disabled,
            ))
            .await
    };
    assert!(
        matches!(result, Err(TransactionError::ParseRejected { ref path, .. }) if path == "second.rs")
    );
    assert_eq!(
        fs::read(fixture.root.join("first.rs")).expect("first"),
        before
    );
    assert!(!fixture.root.join("second.rs").exists());
    assert!(fixture.journal.list(0).expect("no intent").is_empty());
    assert!(fixture.worker.status().await.pid.is_some());
    fixture.worker.shutdown().await.expect("reap parser child");
}

#[tokio::test]
async fn malformed_data_formats_are_rejected_even_without_a_bundled_source_grammar() {
    let mut fixture = Fixture::new();
    for (path, content) in [
        ("data.json", "{\"answer\": }"),
        ("data.yaml", "mapping: [unterminated"),
        ("data.yml", "mapping: [unterminated"),
        ("data.toml", "key = ["),
        ("data.TOML", "key = ["),
    ] {
        let result = {
            let mut transactions = WorkspaceTransactions::new(
                &fixture.policy,
                &fixture.worker,
                &mut fixture.journal,
                &fixture.seal,
                FormatterOptions::default(),
            )
            .expect("transactions");
            transactions
                .prepare(request(
                    vec![change(path, None, Some(content.as_bytes()))],
                    FormatMode::Disabled,
                ))
                .await
        };
        assert!(
            matches!(result, Err(TransactionError::ParseRejected { .. })),
            "malformed {path} needs a well-formedness refusal"
        );
        assert!(!fixture.root.join(path).exists());
    }
    assert!(fixture.journal.list(0).expect("no intent").is_empty());
    fixture.worker.shutdown().await.expect("reap parser child");
}

#[tokio::test]
async fn existing_parse_errors_remain_visible_and_binary_rs_bytes_skip_the_worker() {
    let mut fixture = Fixture::new();
    let binary = [0, 255, 13, 10, 254];
    let text_before = b"pub fn old() {}\n";
    fixture.write("binary.rs", text_before);
    let outcome = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("transactions");
        let prepared = transactions
            .prepare(request(
                vec![change("binary.rs", Some(text_before), Some(&binary))],
                FormatMode::Enabled,
            ))
            .await
            .expect("binary preview");
        assert_eq!(
            prepared.preview()[0]
                .after
                .as_ref()
                .expect("bytes")
                .as_bytes(),
            binary
        );
        assert_eq!(
            fixture.worker.status().await.pid,
            None,
            "binary content starts no parser"
        );
        let token = prepared.confirmation_token().to_owned();
        transactions.apply(prepared, &token).await
    };
    assert_eq!(outcome.status, TransactionStatus::Applied, "{outcome:?}");
    assert_eq!(
        fs::read(fixture.root.join("binary.rs")).expect("binary"),
        binary
    );
    assert_eq!(fixture.worker.status().await.pid, None);

    let before = b"pub fn broken( {\n";
    let candidate = b"// repair in progress\npub fn broken( {\n";
    fixture.write("broken.rs", before);
    let outcome = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("transactions");
        let prepared = transactions
            .prepare(request(
                vec![change("broken.rs", Some(before), Some(candidate))],
                FormatMode::Enabled,
            ))
            .await
            .expect("pre-existing parse errors remain editable");
        assert!(!prepared.preview()[0].diagnostics.is_empty());
        assert_eq!(
            prepared.preview()[0]
                .after
                .as_ref()
                .expect("candidate")
                .as_bytes(),
            candidate
        );
        assert_eq!(
            fs::read(fixture.root.join("broken.rs")).expect("source before apply"),
            before
        );
        let token = prepared.confirmation_token().to_owned();
        transactions.apply(prepared, &token).await
    };
    assert_eq!(outcome.status, TransactionStatus::Applied, "{outcome:?}");
    assert!(!outcome.files[0].preview.diagnostics.is_empty());
    assert_eq!(
        fs::read(fixture.root.join("broken.rs")).expect("source"),
        candidate
    );
    fixture.worker.shutdown().await.expect("reap parser child");
}

#[tokio::test]
async fn a_stopped_parser_refuses_without_journaling_or_touching_files() {
    let mut fixture = Fixture::new();
    fixture.worker.shutdown().await.expect("stop test worker");
    let result = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("transactions");
        transactions
            .prepare(request(
                vec![change("new.rs", None, Some(b"pub fn value() {}\n"))],
                FormatMode::Disabled,
            ))
            .await
    };
    assert!(matches!(result, Err(TransactionError::Worker(_))));
    assert!(!fixture.root.join("new.rs").exists());
    assert!(fixture.journal.list(0).expect("no intent").is_empty());
}

#[tokio::test]
async fn a_real_swap_failure_restores_earlier_files_and_retains_a_rolled_back_intent() {
    let mut fixture = Fixture::new();
    let before = b"pub fn before() {}\n";
    fixture.write("first.rs", before);
    let outcome = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions::default(),
        )
        .expect("transactions");
        let prepared = transactions
            .prepare(request(
                vec![
                    change("first.rs", Some(before), Some(b"pub fn after() {}\n")),
                    change(
                        "missing-parent/second.rs",
                        None,
                        Some(b"pub fn second() {}\n"),
                    ),
                ],
                FormatMode::Disabled,
            ))
            .await
            .expect("the shadow batch parses without creating directories");
        assert!(!fixture.root.join("missing-parent").exists());
        let token = prepared.confirmation_token().to_owned();
        transactions.apply(prepared, &token).await
    };
    assert_eq!(outcome.status, TransactionStatus::Failed, "{outcome:?}");
    assert!(matches!(outcome.error, Some(TransactionError::Io(_))));
    assert_eq!(outcome.files[0].disposition, FileDisposition::RolledBack);
    assert_eq!(outcome.files[1].disposition, FileDisposition::NotApplied);
    assert_eq!(
        fs::read(fixture.root.join("first.rs")).expect("restored first file"),
        before
    );
    assert!(!fixture.root.join("missing-parent").exists());
    let seq = outcome.journal_intent_seq.expect("retained intent");
    assert_eq!(
        fixture
            .journal
            .write_outcome(seq)
            .expect("resolved outcome"),
        Some(JournalWriteOutcome::RolledBack)
    );
    let reader = JournalReader::open(fixture.root.join(".workbench/journal.db"))
        .expect("read-only failure receipt");
    assert_eq!(
        reader.write_outcome(seq).expect("durable failure outcome"),
        Some(JournalWriteOutcome::RolledBack)
    );
    assert!(reader.get(seq).expect("retained operation").is_some());
    assert!(
        reader
            .undo_candidates(&UndoCandidateQuery::default())
            .expect("undo selection")
            .is_empty(),
        "a failed intent never masquerades as applied work to undo"
    );
    fixture.worker.shutdown().await.expect("reap parser child");
}

#[cfg(unix)]
#[tokio::test]
async fn corrupt_formatter_output_is_reverified_by_the_real_parser_and_never_applied() {
    use std::os::unix::fs::PermissionsExt;

    let mut fixture = Fixture::new();
    let formatter = fixture._directory.path().join("corrupt-rustfmt");
    fs::write(
        &formatter,
        "#!/bin/sh\ncat >/dev/null\nprintf 'pub fn broken( {\\n'\n",
    )
    .expect("formatter fixture");
    fs::set_permissions(&formatter, fs::Permissions::from_mode(0o755)).expect("executable fixture");
    let candidate = b"pub fn valid(){}\n";
    let outcome = {
        let mut transactions = WorkspaceTransactions::new(
            &fixture.policy,
            &fixture.worker,
            &mut fixture.journal,
            &fixture.seal,
            FormatterOptions {
                rustfmt: Some(formatter),
                ..Default::default()
            },
        )
        .expect("transactions");
        let prepared = transactions
            .prepare(request(
                vec![change("valid.rs", None, Some(candidate))],
                FormatMode::Enabled,
            ))
            .await
            .expect("verified unformatted fallback");
        assert_eq!(
            prepared.preview()[0]
                .after
                .as_ref()
                .expect("final")
                .as_bytes(),
            candidate
        );
        assert!(
            prepared.preview()[0].diagnostics.is_empty(),
            "candidate is valid"
        );
        assert!(
            prepared.preview()[0].formatting.contains("unformatted"),
            "formatter degradation must be explicit: {:?}",
            prepared.preview()[0]
        );
        let token = prepared.confirmation_token().to_owned();
        transactions.apply(prepared, &token).await
    };
    assert_eq!(outcome.status, TransactionStatus::Applied, "{outcome:?}");
    assert_eq!(
        fs::read(fixture.root.join("valid.rs")).expect("source"),
        candidate
    );
    fixture.worker.shutdown().await.expect("reap parser child");
}
