use super::*;
use std::fs;
use std::time::Instant;

fn await_change(watcher: &IndexWatcher, old: u64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while watcher.epoch() == old && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        watcher.epoch() > old,
        "missing event: {:?}",
        watcher.snapshot()
    );
    // Drain the previous action's native callbacks before the next action;
    // otherwise a delayed write callback could falsely prove a later rename.
    let mut observed = watcher.epoch();
    let mut settled = Instant::now();
    while settled.elapsed() < Duration::from_millis(100) {
        assert!(Instant::now() < deadline, "watcher never settled");
        thread::sleep(Duration::from_millis(10));
        if watcher.epoch() != observed {
            observed = watcher.epoch();
            settled = Instant::now();
        }
    }
    watcher.acknowledge(watcher.epoch());
}

fn exercise_real_changes(mode: WatchMode) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file.rs");
    fs::write(&path, "fn before() {}\n").unwrap();
    let mut watcher =
        IndexWatcher::start(root.path(), mode, false, Duration::from_millis(20)).unwrap();
    // Native backends may finish root registration asynchronously.
    thread::sleep(Duration::from_millis(150));
    let old = watcher.epoch();
    fs::write(&path, "fn after() {}\n").unwrap();
    await_change(&watcher, old);
    let old = watcher.epoch();
    let renamed = root.path().join("renamed.rs");
    fs::rename(&path, &renamed).unwrap();
    await_change(&watcher, old);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let old = watcher.epoch();
        fs::set_permissions(&renamed, fs::Permissions::from_mode(0o700)).unwrap();
        await_change(&watcher, old);
    }
    let old = watcher.epoch();
    fs::remove_file(&renamed).unwrap();
    await_change(&watcher, old);
    let old = watcher.epoch();
    fs::write(root.path().join(".gitignore"), "*.rs\n").unwrap();
    await_change(&watcher, old);
    watcher.shutdown();
    assert!(watcher.snapshot().stopped);
    let stopped = watcher.epoch();
    fs::write(root.path().join("late.rs"), "late").unwrap();
    thread::sleep(Duration::from_millis(100));
    assert_eq!(watcher.epoch(), stopped);
}

#[test]
fn native_edit_rename_mode_delete_and_ignore_changes_are_observed() {
    exercise_real_changes(WatchMode::Native);
}

#[test]
fn polling_edit_rename_mode_delete_and_ignore_changes_are_observed() {
    exercise_real_changes(WatchMode::Poll);
}

#[test]
fn off_mode_has_explicit_inactive_status_and_no_worker() {
    let mut watcher = IndexWatcher::start(
        Path::new("/missing/allowed-when-off"),
        WatchMode::Off,
        true,
        Duration::ZERO,
    )
    .unwrap();
    assert_eq!(watcher.snapshot().mode, WatchMode::Off);
    assert!(!watcher.snapshot().pending);
    assert!(watcher.snapshot().stopped);
    watcher.shutdown();
}

#[test]
fn shutdown_interrupts_long_poll_interval_and_joins_worker() {
    let root = tempfile::tempdir().unwrap();
    let mut watcher = IndexWatcher::start(
        root.path(),
        WatchMode::Auto,
        true,
        Duration::from_secs(3_600),
    )
    .unwrap();
    assert_eq!(watcher.snapshot().mode, WatchMode::Poll);
    assert!(watcher.poller.is_some());
    let start = Instant::now();
    watcher.shutdown();
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(watcher.poller.is_none());
}

#[test]
fn polling_failure_is_visible_and_invalidates_the_cache() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("workspace");
    fs::create_dir(&root).unwrap();
    let watcher =
        IndexWatcher::start(&root, WatchMode::Poll, false, Duration::from_millis(20)).unwrap();
    let old = watcher.epoch();
    fs::remove_dir(&root).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while watcher.snapshot().failures == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let status = watcher.snapshot();
    assert!(status.failures > 0);
    assert!(status.rescans > 0);
    assert!(status.epoch > old);
    assert!(status.last_error.is_some());
}

#[test]
fn unchanged_poll_snapshot_recovers_after_temporary_root_loss() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("workspace");
    fs::create_dir(&root).unwrap();
    let permissions = fs::metadata(&root).unwrap().permissions();
    let watcher =
        IndexWatcher::start(&root, WatchMode::Poll, false, Duration::from_millis(20)).unwrap();
    fs::remove_dir(&root).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while watcher.snapshot().failures == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let failed = watcher.snapshot();
    assert!(failed.last_error.is_some());
    watcher.acknowledge(failed.epoch);
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, permissions).unwrap();
    while watcher.snapshot().last_error.is_some() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let recovered = watcher.snapshot();
    assert!(recovered.last_error.is_none(), "{recovered:?}");
    assert!(recovered.failures >= failed.failures);
    assert!(recovered.epoch > failed.epoch);
    assert!(
        recovered.pending,
        "recovery schedules a fresh reconciliation"
    );
    watcher.acknowledge(recovered.epoch);
    assert!(!watcher.snapshot().pending);
}
