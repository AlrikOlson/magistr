use super::*;

fn lock_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("workspace");
    let state = dir.path().join(".magistr/workspace");
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
fn the_marker_and_lock_inode_survive_release() {
    let (_dir, path) = lock_path();
    let first = ServerLock::acquire(&path).expect("owner");
    let before = std::fs::read(&path).expect("marker");
    assert_eq!(before, NATIVE_MARKER);
    drop(first);
    let second = ServerLock::acquire(&path).expect("next native owner");
    assert_eq!(std::fs::read(&path).expect("unchanged marker"), before);
    drop(second);
    assert!(path.is_file(), "release never unlinks the lock inode");
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
    let path = dir.path().join(".magistr/workspace");
    let refused = ServerLock::acquire(&path).expect_err("a directory is not a lock file");
    assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn unrecognized_records_are_refused_without_modification() {
    for bytes in [b"unknown owner".as_slice(), b"", b"not a lock record"] {
        let (_dir, path) = lock_path();
        std::fs::write(&path, bytes).expect("old owner record");
        let refused = ServerLock::acquire(&path).expect_err("unrecognized marker");
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert!(refused.to_string().contains("unrecognized"));
        assert_eq!(std::fs::read(&path).expect("preserved"), bytes);
    }
}

#[test]
fn simultaneous_first_admission_has_exactly_one_owner() {
    use std::sync::{Arc, Barrier};
    let (_dir, path) = lock_path();
    let ready = Arc::new(Barrier::new(8));
    let attempted = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            let ready = Arc::clone(&ready);
            let attempted = Arc::clone(&attempted);
            std::thread::spawn(move || {
                ready.wait();
                let admission = ServerLock::acquire(path);
                attempted.wait();
                admission.is_ok()
            })
        })
        .collect();
    let owners = threads
        .into_iter()
        .map(|thread| thread.join().expect("contender"))
        .filter(|won| *won)
        .count();
    assert_eq!(owners, 1);
}

#[test]
fn generations_advance_only_on_successful_admission() {
    let (_dir, path) = lock_path();
    let first = ServerLock::acquire(&path).expect("first");
    let generation = first.generation();
    assert!(generation.get() > 0);
    assert!(ServerLock::acquire(&path).is_err());
    drop(first);
    let second = ServerLock::acquire(&path).expect("second");
    assert_eq!(second.generation().get(), generation.get() + 1);
}

#[cfg(unix)]
#[test]
fn directory_aliases_share_the_lock_inode_and_other_workspaces_do_not() {
    use std::os::unix::fs::MetadataExt;
    let (dir, path) = lock_path();
    let alias = dir.path().join("state-alias");
    std::os::unix::fs::symlink(path.parent().expect("state"), &alias).expect("alias");
    let first = ServerLock::acquire(&path).expect("owner");
    let inode = std::fs::metadata(&path).expect("inode").ino();
    assert!(ServerLock::acquire(alias.join(ServerLock::FILE_NAME)).is_err());
    let (_other, other_path) = lock_path();
    let other = ServerLock::acquire(other_path).expect("independent workspace");
    assert_eq!(other.generation().get(), 1);
    drop(first);
    let _next = ServerLock::acquire(&path).expect("reopen");
    assert_eq!(std::fs::metadata(&path).expect("stable inode").ino(), inode);
}

#[cfg(unix)]
#[test]
fn a_held_unknown_marker_refuses_attachment() {
    for bytes in [b"unknown owner".as_slice(), b"", b"not a lock record"] {
        let (_dir, path) = lock_path();
        std::fs::write(&path, bytes).expect("foreign owner marker");
        let holder = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open the foreign marker");
        holder.lock().expect("hold its OS lock");
        let refused = ServerLock::acquire(&path).expect_err("unknown marker is refused");
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert!(matches!(
            super::super::WorkspaceOpenError::lock(refused),
            super::super::WorkspaceOpenError::InvalidLock(_)
        ));
        assert_eq!(std::fs::read(&path).expect("preserved marker"), bytes);
        assert!(
            !path.with_file_name("server.generation").exists(),
            "refusal must not bootstrap native state"
        );
        holder.unlock().expect("release fixture lock");
    }
}

#[test]
fn inspect_classifies_the_record_without_taking_the_lock_or_touching_it() {
    let (_dir, path) = lock_path();
    assert_eq!(
        ServerLock::inspect(&path).expect("absent"),
        LockRecord::Absent
    );
    let held = ServerLock::acquire(&path).expect("owner");
    let before = std::fs::read(&path).expect("marker");
    assert_eq!(
        ServerLock::inspect(&path).expect("native"),
        LockRecord::Native
    );
    // Inspecting beside a live holder neither blocks nor releases it.
    assert_eq!(
        ServerLock::acquire(&path).expect_err("still held").kind(),
        io::ErrorKind::WouldBlock
    );
    drop(held);
    assert_eq!(
        ServerLock::inspect(&path).expect("native"),
        LockRecord::Native
    );
    assert_eq!(std::fs::read(&path).expect("unchanged"), before);

    std::fs::write(&path, b"not a lock record").expect("garbage");
    assert_eq!(
        ServerLock::inspect(&path).expect("unrecognized"),
        LockRecord::Unrecognized
    );
    std::fs::write(&path, b"").expect("empty");
    assert_eq!(
        ServerLock::inspect(&path).expect("empty is unrecognized"),
        LockRecord::Unrecognized
    );
}
