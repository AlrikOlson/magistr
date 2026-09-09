use super::*;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn scan_dir_finds_files() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("foo.rs"), "fn main() {}").unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/lib.rs"), "pub mod foo;").unwrap();

    let snapshot = scan_dir(tmp.path());
    assert!(snapshot.contains_key(Path::new("foo.rs")));
    assert!(snapshot.contains_key(Path::new("src/lib.rs")));
    assert_eq!(snapshot.len(), 2);
}

#[test]
fn scan_dir_skips_excluded() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".git")).unwrap();
    std::fs::write(tmp.path().join(".git/HEAD"), "ref").unwrap();
    std::fs::create_dir_all(tmp.path().join("target")).unwrap();
    std::fs::write(tmp.path().join("target/debug"), "bin").unwrap();
    std::fs::write(tmp.path().join("main.rs"), "fn main() {}").unwrap();

    let snapshot = scan_dir(tmp.path());
    assert_eq!(snapshot.len(), 1);
    assert!(snapshot.contains_key(Path::new("main.rs")));
}

#[test]
fn diff_detects_created() {
    let old = HashMap::new();
    let mut new = HashMap::new();
    let now = SystemTime::now();
    new.insert(PathBuf::from("new_file.rs"), now);

    let events = diff_snapshots(&old, &new);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].path, "new_file.rs");
    assert_eq!(events[0].change, FileChange::Created);
}

#[test]
fn diff_detects_modified() {
    let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
    let t2 = SystemTime::UNIX_EPOCH + Duration::from_secs(2000);

    let mut old = HashMap::new();
    old.insert(PathBuf::from("mod.rs"), t1);

    let mut new = HashMap::new();
    new.insert(PathBuf::from("mod.rs"), t2);

    let events = diff_snapshots(&old, &new);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].change, FileChange::Modified);
}

#[test]
fn diff_detects_deleted() {
    let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);

    let mut old = HashMap::new();
    old.insert(PathBuf::from("gone.rs"), t1);

    let new = HashMap::new();

    let events = diff_snapshots(&old, &new);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].path, "gone.rs");
    assert_eq!(events[0].change, FileChange::Deleted);
}

#[test]
fn diff_no_changes() {
    let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);

    let mut old = HashMap::new();
    old.insert(PathBuf::from("stable.rs"), t1);

    let new = old.clone();

    let events = diff_snapshots(&old, &new);
    assert!(events.is_empty());
}

#[tokio::test]
async fn watcher_detects_new_file() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("existing.rs"), "fn main() {}").unwrap();

    let (cancel, mut event_rx) = start_watcher(tmp.path().to_path_buf());

    // Wait for initial scan
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a new file
    std::fs::write(tmp.path().join("new_file.rs"), "pub fn new() {}").unwrap();

    // Wait for the watcher to detect it
    let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("channel closed");

    assert_eq!(event.path, "new_file.rs");
    assert_eq!(event.change, FileChange::Created);

    // Shut down
    cancel.cancel();
}

// ── FileWatcher trait tests ─────────────────────────────────────────

#[tokio::test]
async fn mock_file_watcher_receives_injected_events() {
    let (mut watcher, tx) = MockFileWatcher::new();
    tx.send(FileEvent {
        path: "src/main.rs".into(),
        change: FileChange::Modified,
    })
    .unwrap();

    let event = watcher.event_rx().recv().await.unwrap();
    assert_eq!(event.path, "src/main.rs");
    assert_eq!(event.change, FileChange::Modified);
}

#[tokio::test]
async fn mock_file_watcher_multiple_events() {
    let (mut watcher, tx) = MockFileWatcher::new();
    tx.send(FileEvent {
        path: "a.rs".into(),
        change: FileChange::Created,
    })
    .unwrap();
    tx.send(FileEvent {
        path: "b.rs".into(),
        change: FileChange::Deleted,
    })
    .unwrap();

    let e1 = watcher.event_rx().recv().await.unwrap();
    let e2 = watcher.event_rx().recv().await.unwrap();
    assert_eq!(e1.change, FileChange::Created);
    assert_eq!(e2.change, FileChange::Deleted);
}

#[tokio::test]
async fn polling_file_watcher_as_trait_object() {
    let tmp = TempDir::new().unwrap();
    let watcher = PollingFileWatcher::new(tmp.path().to_path_buf());
    // Verify it implements the trait (used as Box<dyn FileWatcher>)
    let _: Box<dyn FileWatcher> = Box::new(watcher);
}

#[tokio::test]
async fn mock_file_watcher_as_trait_object() {
    let (watcher, tx) = MockFileWatcher::new();
    let mut boxed: Box<dyn FileWatcher> = Box::new(watcher);
    tx.send(FileEvent {
        path: "test.rs".into(),
        change: FileChange::Created,
    })
    .unwrap();

    let event = boxed.event_rx().recv().await.unwrap();
    assert_eq!(event.path, "test.rs");
}

#[test]
fn workbench_state_directory_is_skipped_like_git_and_target() {
    for dir in [".git", "target", ".workbench"] {
        assert!(SKIP_DIRS.contains(&dir), "{dir} must be skipped");
    }
}
