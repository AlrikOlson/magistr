use super::*;

fn lock_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("workspace");
    let state = dir.path().join(".workbench");
    std::fs::create_dir_all(&state).expect("state dir");
    let path = state.join(ServerLock::FILE_NAME);
    (dir, path)
}

#[test]
fn a_second_acquire_is_refused_until_the_holder_releases() {
    let (_dir, path) = lock_path();
    let first = ServerLock::acquire(&path).expect("first owner");
    assert_eq!(first.path(), path);
    let refused = ServerLock::acquire(&path).expect_err("a live holder refuses");
    assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    assert!(refused.to_string().contains("already owns this workspace"));
    drop(first);
    ServerLock::acquire(&path).expect("released by the drop, not by age");
}

#[test]
fn the_lock_file_survives_release_and_carries_no_pid() {
    let (_dir, path) = lock_path();
    drop(ServerLock::acquire(&path).expect("owner"));
    assert!(
        path.is_file(),
        "the lock file is an OS lock target, not a record"
    );
    assert_eq!(std::fs::read(&path).expect("bytes"), b"");
}

#[cfg(unix)]
#[test]
fn a_symlinked_lock_path_is_refused() {
    let (dir, path) = lock_path();
    let elsewhere = dir.path().join("elsewhere.lock");
    std::fs::write(&elsewhere, b"").expect("target");
    std::os::unix::fs::symlink(&elsewhere, &path).expect("alias");
    let refused = ServerLock::acquire(&path).expect_err("aliases are refused");
    assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn a_directory_at_the_lock_path_is_refused() {
    let (dir, _) = lock_path();
    let path = dir.path().join(".workbench");
    let refused = ServerLock::acquire(&path).expect_err("a directory is not a lock file");
    assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
}
