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
async fn status_is_byte_identical_across_fresh_owners_and_wears_the_retained_row() {
    let dir = workspace();
    let mut sealed = Vec::new();
    for _ in 0..2 {
        let service = WorkspaceService::open(options(dir.path()))
            .await
            .expect("owner");
        sealed.push(
            ResponseAdapter::seal(&service.status())
                .expect("sealed")
                .json()
                .to_owned(),
        );
        service.shutdown().await.expect("shutdown");
    }
    assert_eq!(sealed[0], sealed[1]);
    let envelope: Value = serde_json::from_str(&sealed[0]).expect("json");
    let row = &envelope["results"][0];
    assert_eq!(row["journalMode"], "WAL");
    assert_eq!(
        row["appliedJournalMode"], "wal",
        "SQLite's own answer, verbatim"
    );
    assert_eq!(row["networkFs"], false);
    assert_eq!(row["detectionMethod"], "env-override");
    assert_eq!(row["envelope"], serde_json::json!({"min": 1, "latest": 1}));
    assert!(
        envelope.get("notes").is_none(),
        "a healthy local workspace has no notes"
    );
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
    let WorkspaceResponse::Success { results, notes, .. } =
        service.journal_list(&JournalListInput::default(), "").await
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
    let WorkspaceResponse::Success { results, notes, .. } = service.status() else {
        panic!("status");
    };
    assert_eq!(results.rows()[0].journal_mode, "TRUNCATE");
    assert_eq!(results.rows()[0].applied_journal_mode, "truncate");
    let notes = notes.expect("degradation note");
    assert!(notes[0].contains("degraded WAL→TRUNCATE"));
    let WorkspaceResponse::Success { notes, .. } =
        service.journal_list(&JournalListInput::default(), "").await
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
