use super::*;
use crate::response::ResponseAdapter;

/// A worker executable that is never spawned: these tests read status,
/// list the journal and exercise admission over workspaces with nothing to
/// parse. The supervisor is lazy, so the path is never resolved.
fn options(root: &Path) -> WorkspaceOpenOptions {
    WorkspaceOpenOptions::new(root, "/nonexistent/magistr-worker").with_network_override("0")
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(dir.path().join(".git")).expect("repository marker");
    dir
}

#[tokio::test]
async fn read_source_preserves_complete_text_and_binary_bytes_without_a_parser() {
    let dir = workspace();
    std::fs::create_dir(dir.path().join("data")).expect("source directory");
    let text = format!("{}last line", "first\r\n😀\n".repeat(10_000));
    let cases = [
        ("data/text.txt", text.as_bytes()),
        ("data/binary.bin", &[0, 0xff, 0xfe, b'\r', b'\n'][..]),
        ("data/empty.txt", &[][..]),
    ];
    for (path, bytes) in &cases {
        std::fs::write(dir.path().join(path), bytes).expect("source");
    }
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    for (path, bytes) in cases {
        let source = service
            .read_source(&path.replace('/', "\\"), None)
            .expect("exact source");
        assert_eq!(source.path, path);
        assert_eq!(source.content.as_bytes(), bytes);
        assert_eq!(
            source.content_hash,
            crate::file_content::FileContent::decode(bytes.to_vec()).content_hash()
        );
        assert_eq!(source.freshness, Freshness::Warm);
        assert_eq!(
            matches!(source.content, crate::file_content::FileContent::Binary(_)),
            path.ends_with(".bin")
        );
    }
    assert!(service.worker_status().await.pid.is_none());
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn read_source_revalidates_the_hash_and_reports_a_deleted_selection() {
    let dir = workspace();
    let path = dir.path().join("selected.txt");
    std::fs::write(&path, b"first").expect("source");
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    let first = service
        .read_source("selected.txt", None)
        .expect("selection");
    let unchanged = service
        .read_source("selected.txt", Some(&first.content_hash))
        .expect("same source");
    assert_eq!(unchanged.content, first.content);

    std::fs::write(&path, b"other").expect("same-length edit");
    let fresh = service
        .read_source("selected.txt", None)
        .expect("new source");
    let stale = service
        .read_source("selected.txt", Some(&first.content_hash))
        .expect_err("a changed selection is refused");
    assert_eq!(
        stale.kind,
        ErrorKind::StaleAnchor {
            content_hash: fresh.content_hash,
        }
    );
    std::fs::remove_file(path).expect("delete source");
    assert_eq!(
        service
            .read_source("selected.txt", Some(&first.content_hash))
            .expect_err("a deleted selection is refused")
            .kind,
        ErrorKind::FileNotFound
    );
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn read_source_enforces_the_live_byte_ceiling_and_workspace_boundary() {
    let dir = workspace();
    let outside = workspace();
    let outside_path = outside.path().join("secret.txt");
    std::fs::write(&outside_path, b"private").expect("outside source");
    let path = dir.path().join("limit.bin");
    let file = std::fs::File::create(&path).expect("bounded source");
    file.set_len(1024 * 1024).expect("at ceiling");
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    assert_eq!(
        service
            .read_source("limit.bin", None)
            .expect("the ceiling is inclusive")
            .content
            .as_bytes()
            .len(),
        1024 * 1024
    );
    file.set_len(1024 * 1024 + 1).expect("past ceiling");
    for (path, expected) in [
        ("limit.bin", ErrorKind::BudgetTooSmall),
        ("missing.txt", ErrorKind::FileNotFound),
        (".", ErrorKind::FileNotFound),
        ("../escape.txt", ErrorKind::PathOutsideWorkspace),
        (
            outside_path.to_str().expect("UTF-8 fixture path"),
            ErrorKind::PathOutsideWorkspace,
        ),
    ] {
        assert_eq!(
            service.read_source(path, None).expect_err("refused").kind,
            expected,
            "{path}"
        );
    }
    service.shutdown().await.expect("shutdown");
}

#[cfg(unix)]
#[tokio::test]
async fn read_source_refuses_a_symlink_to_another_workspace() {
    let dir = workspace();
    let outside = workspace();
    let outside_path = outside.path().join("secret.txt");
    std::fs::write(&outside_path, b"private").expect("outside source");
    std::os::unix::fs::symlink(&outside_path, dir.path().join("escape.txt")).expect("symlink");
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    assert_eq!(
        service
            .read_source("escape.txt", None)
            .expect_err("outside source is refused")
            .kind,
        ErrorKind::PathOutsideWorkspace
    );
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_missing_root_is_refused_with_the_retained_exit_code() {
    let dir = workspace();
    let missing = dir.path().join("absent");
    let refused = WorkspaceService::open(options(&missing))
        .await
        .err()
        .expect("no root, no workspace");
    assert!(matches!(refused, WorkspaceOpenError::RootMissing(_)));
    assert_eq!(refused.exit_code(), 66);
    assert!(
        refused
            .to_string()
            .starts_with("workspace root does not exist: ")
    );
}

#[tokio::test]
async fn a_second_owner_is_refused_while_the_first_is_alive() {
    let dir = workspace();
    let first = WorkspaceService::open(options(dir.path()))
        .await
        .expect("first owner");
    let refused = WorkspaceService::open(options(dir.path()))
        .await
        .err()
        .expect("the OS lock refuses a second owner");
    assert!(matches!(refused, WorkspaceOpenError::Locked(_)));
    assert_eq!(refused.exit_code(), 75);
    first.shutdown().await.expect("clean shutdown");
    let again = WorkspaceService::open(options(dir.path()))
        .await
        .expect("released by shutdown, not by age");
    again.shutdown().await.expect("clean shutdown");
}

#[tokio::test]
async fn status_observes_the_owned_runtime_and_changes_when_the_worker_stops() {
    let dir = workspace();
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    let response = service.status().await;
    let sealed = ResponseAdapter::seal(&response).expect("sealed");
    let envelope = sealed.structured();
    let row = &envelope["results"][0];
    assert_eq!(row["journalMode"], "WAL");
    assert_eq!(row["appliedJournalMode"], "wal", "SQLite's measured answer");
    assert_eq!(row["networkFs"], false);
    assert_eq!(row["detectionMethod"], "env-override");
    assert_eq!(row["envelope"], serde_json::json!({"min": 1, "latest": 1}));
    assert_eq!(row["worker"]["stopped"], false);
    assert_eq!(
        row["worker"]["pid"],
        Value::Null,
        "observation does not start the worker"
    );
    assert_eq!(row["worker"]["quarantined"], 0);
    assert!(row["index"]["generation"].is_u64());
    assert!(row["index"]["freshness"].is_string());

    service
        .worker
        .shutdown()
        .await
        .expect("stop the owned worker");
    let WorkspaceResponse::Success { results, notes, .. } = service.status().await else {
        panic!("status");
    };
    assert!(results.rows()[0].worker.stopped);
    assert!(
        notes
            .expect("stopped worker")
            .iter()
            .any(|note| note.contains("worker stopped"))
    );
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn admission_creates_the_state_dir_key_and_journal_and_the_listing_starts_empty() {
    let dir = workspace();
    let service = WorkspaceService::open(options(dir.path()))
        .await
        .expect("owner");
    let state = service.state_dir().to_path_buf();
    assert!(state.join("server.lock").is_file());
    assert!(state.join("keys/workspace.key").is_file());
    assert!(state.join("journal.db").is_file());
    assert!(state.join("trace.jsonl").exists() || service.trace().is_enabled());
    assert!(service.elicitation_allowed());
    // The first build is kicked at open; the task derives from its status.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let task = loop {
        if let Some(task) = service.index_task() {
            break task;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no build was ever kicked: {:?}",
            service.index.status()
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    assert!(task.created_at_ms > 0);
    let WorkspaceResponse::Success { results, notes, .. } = service
        .journal_list(
            &JournalListInput::default(),
            &service.admit_session("test").expect("session"),
        )
        .await
    else {
        panic!("listing expected");
    };
    assert!(results.rows().is_empty());
    assert_eq!(
        notes,
        Some(vec![
            "undo: the journal has no ops in workspace scope".to_owned()
        ])
    );
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_token_confirm_mode_disables_elicitation_and_flags_ride_the_config() {
    let dir = workspace();
    let mut flags = serde_json::Map::new();
    flags.insert("edit.confirm_mode".into(), Value::String("token".into()));
    flags.insert("tracing.enabled".into(), Value::Bool(false));
    let service = WorkspaceService::open(options(dir.path()).with_session_flags(flags))
        .await
        .expect("owner");
    assert!(!service.elicitation_allowed());
    assert!(!service.trace().is_enabled());
    assert!(!service.offers_script());
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_network_workspace_degrades_to_truncate_and_says_so_on_every_envelope() {
    let dir = workspace();
    let service = WorkspaceService::open(
        WorkspaceOpenOptions::new(dir.path(), "/nonexistent/magistr-worker")
            .with_network_override("1"),
    )
    .await
    .expect("owner");
    let WorkspaceResponse::Success { results, notes, .. } = service.status().await else {
        panic!("status");
    };
    assert_eq!(results.rows()[0].journal_mode, "TRUNCATE");
    assert_eq!(results.rows()[0].applied_journal_mode, "truncate");
    let notes = notes.expect("degradation note");
    assert!(notes[0].contains("degraded WAL→TRUNCATE"));
    let WorkspaceResponse::Success { notes, .. } = service
        .journal_list(
            &JournalListInput::default(),
            &service.admit_session("test").expect("session"),
        )
        .await
    else {
        panic!("listing");
    };
    let notes = notes.expect("notes");
    assert!(
        notes
            .last()
            .expect("workspace note last")
            .contains("network filesystem")
    );
    service.shutdown().await.expect("shutdown");
}
