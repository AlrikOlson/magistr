use super::*;
use std::fs;

#[test]
fn generation_is_stable_across_loads_and_rotation_invalidates_old_identity() {
    let root = tempfile::tempdir().unwrap();
    let store = WorkspaceKeyStore::new(root.path().join(".workbench"));
    let first = store.ensure().unwrap();
    assert_eq!(fs::read(store.path()).unwrap(), first.as_bytes());
    assert!(first == store.ensure().unwrap());
    assert!(
        first
            == WorkspaceKeyStore::new(root.path().join(".workbench"))
                .ensure()
                .unwrap()
    );
    let rotated = store.rotate().unwrap();
    assert!(rotated != first);
    assert!(rotated == store.ensure().unwrap());
}

#[test]
fn malformed_existing_material_is_preserved_until_explicit_rotation() {
    let root = tempfile::tempdir().unwrap();
    let store = WorkspaceKeyStore::new(root.path());
    store.ensure().unwrap();
    fs::write(store.path(), b"broken").unwrap();
    assert!(matches!(
        store.ensure(),
        Err(KeyError::Malformed { length: 6, .. })
    ));
    assert_eq!(fs::read(store.path()).unwrap(), b"broken");
    assert_eq!(store.rotate().unwrap().as_bytes().len(), 32);
}

#[test]
fn oversized_existing_key_is_rejected_without_reading_or_replacing_its_content() {
    let root = tempfile::tempdir().unwrap();
    let store = WorkspaceKeyStore::new(root.path());
    store.ensure().unwrap();
    let length = 1_073_741_824;
    fs::OpenOptions::new()
        .write(true)
        .open(store.path())
        .unwrap()
        .set_len(length)
        .unwrap();
    assert!(matches!(
        store.ensure(),
        Err(KeyError::Malformed {
            length: 1_073_741_824,
            ..
        })
    ));
    assert_eq!(fs::metadata(store.path()).unwrap().len(), length);
}

#[test]
fn concurrent_first_use_never_overwrites_another_generated_key() {
    let root = tempfile::tempdir().unwrap();
    let state = root.path().to_path_buf();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let keys = std::thread::scope(|scope| {
        (0..8)
            .map(|_| {
                let barrier = &barrier;
                let state = &state;
                scope.spawn(move || {
                    barrier.wait();
                    WorkspaceKeyStore::new(state).ensure().unwrap()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(keys.iter().all(|key| key == &keys[0]));
}

#[cfg(unix)]
#[test]
fn owner_modes_are_preserved_after_rotation() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let store = WorkspaceKeyStore::new(root.path());
    store.ensure().unwrap();
    assert_eq!(
        fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(root.path().join("keys"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    store.rotate().unwrap();
    assert_eq!(
        fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[test]
fn key_and_directory_symlinks_are_refused_without_touching_their_targets() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let store = WorkspaceKeyStore::new(root.path());
    fs::create_dir(root.path().join("keys")).unwrap();
    let target = outside.path().join("key");
    fs::write(&target, [7; 32]).unwrap();
    symlink(&target, store.path()).unwrap();
    assert!(matches!(store.ensure(), Err(KeyError::UnsafeLocation(_))));
    assert!(matches!(store.rotate(), Err(KeyError::UnsafeLocation(_))));
    assert_eq!(fs::read(&target).unwrap(), [7; 32]);
    fs::remove_file(store.path()).unwrap();
    fs::remove_dir(root.path().join("keys")).unwrap();
    symlink(outside.path(), root.path().join("keys")).unwrap();
    assert!(matches!(store.ensure(), Err(KeyError::UnsafeLocation(_))));
    assert!(matches!(store.rotate(), Err(KeyError::UnsafeLocation(_))));
}
