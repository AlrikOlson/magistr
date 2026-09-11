use super::*;
use crate::service::ServerLock;

fn lock_path() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("workspace state");
    let path = dir.path().join(ServerLock::FILE_NAME);
    (dir, path)
}

#[test]
fn admissions_advance_and_keep_one_stable_lock_inode() {
    let (_dir, path) = lock_path();
    let first = ServerLock::acquire(&path).expect("first owner");
    assert_eq!(first.generation(), WorkspaceGeneration::new(1));
    let marker = std::fs::read(&path).expect("marker");
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&path).expect("lock inode").ino()
    };
    drop(first);
    let second = ServerLock::acquire(&path).expect("second owner");
    assert_eq!(second.generation().get(), 2);
    assert_eq!(std::fs::read(&path).expect("same marker"), marker);
    assert_eq!(
        std::fs::read(WorkspaceGeneration::path(&path)).expect("counter"),
        b"00000000000000000002"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&path).expect("same lock").ino(), inode);
    }
}

#[test]
fn interrupted_bootstrap_resumes_without_resetting_an_admitted_generation() {
    let (_dir, path) = lock_path();
    WorkspaceGeneration::bootstrap(&path).expect("zero published before marker");
    assert!(!path.exists());
    let owner = ServerLock::acquire(&path).expect("resume initial creation");
    assert_eq!(owner.generation().get(), 1);
    drop(owner);
    std::fs::remove_file(&path).expect("simulate missing lock while offline");
    let error = ServerLock::acquire(&path).expect_err("do not replace missing admitted lock");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(!path.exists());
}

#[test]
fn a_late_bootstrap_contender_rechecks_the_published_lock() {
    let (_dir, path) = lock_path();
    let owner = ServerLock::acquire(&path).expect("winner published marker and generation");
    let before = std::fs::read(WorkspaceGeneration::path(&path)).expect("generation");
    WorkspaceGeneration::bootstrap(&path).expect("a stale absence observation is not corruption");
    assert_eq!(
        std::fs::read(WorkspaceGeneration::path(&path)).expect("unchanged"),
        before
    );
    assert_eq!(
        ServerLock::acquire(&path)
            .expect_err("ordinary contention")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(owner);
}

#[test]
fn an_existing_marker_never_resets_a_missing_counter() {
    let (_dir, path) = lock_path();
    drop(ServerLock::acquire(&path).expect("initial owner"));
    std::fs::remove_file(WorkspaceGeneration::path(&path)).expect("simulate lost generation");
    let error = ServerLock::acquire(&path).expect_err("missing generation refuses");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("must not be reset"));
    assert!(!WorkspaceGeneration::path(&path).exists());
}

#[test]
fn corrupt_or_exhausted_counters_refuse_without_changing_any_bytes() {
    let (_dir, path) = lock_path();
    drop(ServerLock::acquire(&path).expect("initial owner"));
    let generation = WorkspaceGeneration::path(&path);
    for bytes in [
        b"".as_slice(),
        b"1",
        b"00000000000000000001\n",
        b"0000000000000000000x",
        b"99999999999999999999",
        b"18446744073709551615",
    ] {
        std::fs::write(&generation, bytes).expect("invalid counter fixture");
        let error = ServerLock::acquire(&path).expect_err("strict generation refusal");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            std::fs::read(&generation).expect("unchanged counter"),
            bytes
        );
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_counter_is_refused_without_touching_its_target() {
    let (dir, path) = lock_path();
    let target = dir.path().join("elsewhere");
    std::fs::write(&target, b"00000000000000000000").expect("target");
    std::os::unix::fs::symlink(&target, WorkspaceGeneration::path(&path)).expect("counter alias");
    let error = ServerLock::acquire(&path).expect_err("no counter aliases");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        std::fs::read(&target).expect("target unchanged"),
        b"00000000000000000000"
    );
    assert!(!path.exists());
}
