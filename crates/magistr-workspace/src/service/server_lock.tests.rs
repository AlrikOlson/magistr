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
fn the_native_marker_survives_release_and_is_readable_to_legacy_clients() {
    let (_dir, path) = lock_path();
    let first = ServerLock::acquire(&path).expect("owner");
    let before = std::fs::read(&path).expect("marker");
    let marker: serde_json::Value = serde_json::from_slice(&before).expect("legacy JSON");
    assert!(marker["pid"].is_number());
    assert_eq!(marker["host"], "magistr-native-os-lock/v1");
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
    let path = dir.path().join(".workbench");
    let refused = ServerLock::acquire(&path).expect_err("a directory is not a lock file");
    assert_eq!(refused.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn legacy_and_unrecognized_records_are_refused_without_modification() {
    for bytes in [
        br#"{"pid":123,"host":"legacy-host"}"#.as_slice(),
        b"",
        b"not a lock record",
    ] {
        let (_dir, path) = lock_path();
        std::fs::write(&path, bytes).expect("old owner record");
        let refused = ServerLock::acquire(&path).expect_err("requires offline migration");
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert!(refused.to_string().contains("migration"));
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
fn a_held_legacy_or_unknown_marker_requires_migration_instead_of_native_attachment() {
    for bytes in [
        br#"{"pid":123,"host":"legacy-host"}"#.as_slice(),
        b"",
        b"not a lock record",
    ] {
        let (_dir, path) = lock_path();
        std::fs::write(&path, bytes).expect("foreign owner marker");
        let holder = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .expect("open the foreign marker");
        holder.lock().expect("hold its OS lock");
        let refused = ServerLock::acquire(&path).expect_err("migration remains mandatory");
        assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
        assert!(matches!(
            super::super::WorkspaceOpenError::lock(refused),
            super::super::WorkspaceOpenError::MigrationRequired(_)
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
fn migration_leaves_absent_and_native_markers_alone() {
    let (_dir, path) = lock_path();
    assert_eq!(
        ServerLock::migrate_legacy(&path).expect("nothing to do"),
        LegacyMigration::Absent
    );
    let held = ServerLock::acquire(&path).expect("native owner");
    let marker = std::fs::read(&path).expect("native marker");
    assert_eq!(
        ServerLock::migrate_legacy(&path).expect("already native"),
        LegacyMigration::AlreadyNative
    );
    assert_eq!(std::fs::read(&path).expect("untouched"), marker);
    drop(held);
    assert_eq!(
        ServerLock::migrate_legacy(&path).expect("still native after release"),
        LegacyMigration::AlreadyNative
    );
}

#[cfg(unix)]
#[test]
fn migration_refuses_a_live_legacy_owner_and_a_foreign_host() {
    let (_dir, path) = lock_path();
    let alive = format!(
        r#"{{"pid":{},"host":"{}"}}"#,
        std::process::id(),
        local_hostname()
    );
    std::fs::write(&path, &alive).expect("legacy record");
    let refused = ServerLock::migrate_legacy(&path).expect_err("its process is this one");
    assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    assert!(refused.to_string().contains("alive"), "{refused}");
    assert_eq!(std::fs::read_to_string(&path).expect("preserved"), alive);

    let foreign = r#"{"pid":1,"host":"another-machine.local"}"#;
    std::fs::write(&path, foreign).expect("foreign record");
    let refused = ServerLock::migrate_legacy(&path).expect_err("another host wrote it");
    assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    assert!(
        refused.to_string().contains("another-machine.local"),
        "{refused}"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("preserved"), foreign);
}

#[cfg(unix)]
#[test]
fn migration_moves_a_dead_legacy_record_or_an_empty_lock_aside_and_admission_follows() {
    let mut child = std::process::Command::new("true")
        .spawn()
        .expect("a process to outlive");
    let dead = child.id();
    child
        .wait()
        .expect("reaped: the pid is gone before the record names it");
    let (_dir, path) = lock_path();
    let record = format!(r#"{{"pid":{dead},"host":"{}"}}"#, local_hostname());
    std::fs::write(&path, &record).expect("legacy record");
    let LegacyMigration::Migrated { backup } =
        ServerLock::migrate_legacy(&path).expect("dead owner migrates")
    else {
        panic!("a dead legacy owner is migrated");
    };
    assert!(!path.exists(), "the record is moved, not copied");
    assert_eq!(std::fs::read_to_string(&backup).expect("backup"), record);
    let admitted = ServerLock::acquire(&path).expect("native admission after migration");
    assert_eq!(admitted.generation().get(), 1);
    drop(admitted);

    let (_dir, path) = lock_path();
    std::fs::write(&path, b"").expect("empty pre-migration lock");
    assert!(matches!(
        ServerLock::migrate_legacy(&path).expect("empty lock migrates"),
        LegacyMigration::Migrated { .. }
    ));
    ServerLock::acquire(&path).expect("native admission after migration");
}
