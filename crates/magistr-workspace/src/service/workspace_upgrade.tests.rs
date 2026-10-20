use super::*;
use crate::{
    file_content::FileContent,
    journal::{
        JournalDbOptions, JournalFileChange, JournalFileKind, JournalLogQuery, JournalOpInit,
        JournalReader,
    },
    service::{WorkspaceOpenOptions, WorkspaceService},
};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join(".git")).unwrap();
    std::fs::create_dir(root.join(".workbench")).unwrap();
    let path = root.join(".workbench/journal.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch(include_str!("../../fixtures/upgrades/workbench-v3.sql"))
        .unwrap();
    drop(db);
    let mut journal = JournalDb::open(JournalDbOptions::new(&path)).unwrap();
    journal
        .append(&JournalOpInit {
            tool: "edit".into(),
            args_hash: "args".into(),
            session_id: "saved-session".into(),
            undo_of: None,
            files: vec![JournalFileChange {
                kind: JournalFileKind::Write,
                path: "file.txt".into(),
                before: FileContent::Text("before".into()),
                after: FileContent::Text("after".into()),
            }],
        })
        .unwrap();
    drop(journal);
    drop(ServerLock::acquire(root.join(".workbench/server.lock")).unwrap());
    dir
}

fn options(root: &Path) -> WorkspaceOpenOptions {
    WorkspaceOpenOptions::new(root, "/nonexistent/upgrade-test-worker").with_network_override("0")
}

#[tokio::test]
async fn ordinary_admission_moves_settings_and_history_without_reusing_transient_state() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("workbench.toml"), "[commands]\ntest='true'\n").unwrap();
    std::fs::write(root.join(".workbench/native-index.db"), "discardable index").unwrap();
    std::fs::write(root.join(".workbench/trace.jsonl"), "old trace").unwrap();
    let old = JournalReader::open(root.join(".workbench/journal.db"))
        .unwrap()
        .list(0)
        .unwrap();
    let service = WorkspaceService::open(options(root)).await.unwrap();
    let current = JournalReader::open(root.join(".magistr/workspace/journal.db"))
        .unwrap()
        .list(0)
        .unwrap();
    assert_eq!(current, old);
    assert_eq!(current[0].session_id, "saved-session");
    assert!(!root.join("workbench.toml").exists());
    assert!(root.join(".magistr/workspace.toml").exists());
    assert!(
        root.join(".workbench/journal.db").exists(),
        "recoverable original retained"
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".workbench/native-index.db")).unwrap(),
        "discardable index"
    );
    assert!(
        ServerLock::acquire(root.join(".workbench/server.lock")).is_err(),
        "older binary cannot reopen a retired store"
    );
    service.shutdown().await.unwrap();
    let second = WorkspaceService::open(options(root)).await.unwrap();
    assert_eq!(
        JournalReader::open(root.join(".magistr/workspace/journal.db"))
            .unwrap()
            .list(0)
            .unwrap(),
        old
    );
    second.shutdown().await.unwrap();
}

#[tokio::test]
async fn live_older_owner_is_respected_before_configuration_or_journal_changes() {
    let dir = fixture();
    let root = dir.path();
    let owner = ServerLock::acquire(root.join(".workbench/server.lock")).unwrap();
    std::fs::write(root.join("workbench.toml"), "[commands]\ntest='true'\n").unwrap();
    let error = WorkspaceService::open(options(root)).await.err().unwrap();
    assert_eq!(error.exit_code(), 75);
    assert!(error.to_string().contains("Close that session"), "{error}");
    assert!(!root.join(".magistr/workspace/journal.db").exists());
    assert!(!root.join(".magistr/workspace.toml").exists());
    assert!(root.join("workbench.toml").exists());
    drop(owner);
    WorkspaceService::open(options(root))
        .await
        .unwrap()
        .shutdown()
        .await
        .unwrap();
}

#[test]
fn committed_import_recovers_a_torn_retirement_without_duplicate_rows() {
    let dir = fixture();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".magistr/workspace")).unwrap();
    let _owner = ServerLock::acquire(root.join(".magistr/workspace/server.lock")).unwrap();
    let current = JournalDb::open(JournalDbOptions::new(
        root.join(".magistr/workspace/journal.db"),
    ))
    .unwrap();
    let older = WorkspaceUpgrade::acquire(root).unwrap().unwrap();
    current
        .import_workbench(&root.join(".workbench/journal.db"))
        .unwrap();
    drop(older); // crash window: transaction committed, marker not yet changed
    let mut marker = ORIGINAL.to_vec();
    marker[..13].copy_from_slice(&RETIRED[..13]);
    std::fs::write(root.join(".workbench/server.lock"), marker).unwrap();
    WorkspaceUpgrade::acquire(root)
        .unwrap()
        .unwrap()
        .finish(&current)
        .unwrap();
    assert_eq!(
        current.recent(&JournalLogQuery::default()).unwrap().len(),
        1
    );
    WorkspaceUpgrade::acquire(root)
        .unwrap()
        .unwrap()
        .finish(&current)
        .unwrap();
    assert_eq!(current.list(0).unwrap().len(), 1);
}

#[test]
fn unrecognized_ownership_is_not_treated_as_an_abandoned_lock() {
    let dir = fixture();
    let root = dir.path();
    let bytes = b"{\"pid\":1234,\"host\":\"unknown\"}";
    std::fs::write(root.join(".workbench/server.lock"), bytes).unwrap();
    assert!(WorkspaceUpgrade::acquire(root).is_err());
    assert_eq!(
        std::fs::read(root.join(".workbench/server.lock")).unwrap(),
        bytes
    );
    assert_eq!(
        JournalReader::open(root.join(".workbench/journal.db"))
            .unwrap()
            .list(0)
            .unwrap()
            .len(),
        1
    );
}

#[cfg(unix)]
#[test]
fn older_state_directory_and_journal_links_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let external = fixture();
    std::os::unix::fs::symlink(
        external.path().join(".workbench"),
        dir.path().join(".workbench"),
    )
    .unwrap();
    assert!(WorkspaceUpgrade::acquire(dir.path()).is_err());
    let dir = fixture();
    std::fs::remove_file(dir.path().join(".workbench/journal.db")).unwrap();
    std::os::unix::fs::symlink(
        external.path().join(".workbench/journal.db"),
        dir.path().join(".workbench/journal.db"),
    )
    .unwrap();
    assert!(WorkspaceUpgrade::acquire(dir.path()).is_err());
}
