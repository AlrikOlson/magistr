use super::*;
use std::fs;

#[test]
fn content_changes_with_restored_mtime_and_length_change_digest() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file.rs");
    fs::write(&path, "pub fn old() {}\n").unwrap();
    // Restoring the timestamp needs write access to the handle on Windows.
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let mtime = file.metadata().unwrap().modified().unwrap();
    let stop = AtomicBool::new(false);
    let before = PollingDigest::capture(root.path(), &stop).unwrap();
    fs::write(&path, "pub fn new() {}\n").unwrap();
    file.set_modified(mtime).unwrap();
    assert_ne!(before, PollingDigest::capture(root.path(), &stop).unwrap());
}

#[test]
fn byte_and_entry_limits_fail_without_publishing_partial_snapshot() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file.rs"), "12345").unwrap();
    let stop = AtomicBool::new(false);
    let error = PollingDigest::capture_with_limits(root.path(), &stop, 10, 4).unwrap_err();
    assert!(error.to_string().contains("byte limit"));
    let error = PollingDigest::capture_with_limits(root.path(), &stop, 0, 100).unwrap_err();
    assert!(error.to_string().contains("entry limit"));
    stop.store(true, Ordering::Release);
    assert_eq!(
        PollingDigest::capture(root.path(), &stop)
            .unwrap_err()
            .kind(),
        io::ErrorKind::Interrupted
    );
}

#[test]
fn cache_and_gitignored_files_do_not_trigger_content_polling() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".workbench")).unwrap();
    fs::create_dir(root.path().join(".git")).unwrap();
    fs::write(root.path().join(".gitignore"), "ignored.rs\n").unwrap();
    fs::write(root.path().join("ignored.rs"), "before").unwrap();
    let stop = AtomicBool::new(false);
    let before = PollingDigest::capture(root.path(), &stop).unwrap();
    fs::write(root.path().join("ignored.rs"), "after").unwrap();
    fs::write(root.path().join(".workbench/index.db"), "cache").unwrap();
    assert_eq!(before, PollingDigest::capture(root.path(), &stop).unwrap());
    fs::write(root.path().join(".gitignore"), "").unwrap();
    assert_ne!(before, PollingDigest::capture(root.path(), &stop).unwrap());
}

#[cfg(unix)]
#[test]
fn polling_never_hashes_external_symlink_contents() {
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let target = external.path().join("private.rs");
    fs::write(&target, "before").unwrap();
    std::os::unix::fs::symlink(&target, root.path().join("link.rs")).unwrap();
    let stop = AtomicBool::new(false);
    let before = PollingDigest::capture(root.path(), &stop).unwrap();
    fs::write(target, "after").unwrap();
    assert_eq!(before, PollingDigest::capture(root.path(), &stop).unwrap());
}
