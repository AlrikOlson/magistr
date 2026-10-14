use super::*;

#[test]
fn overlapping_replacements_own_different_temporary_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let first = TemporaryFile::create(dir.path(), path.file_name().unwrap()).unwrap();
    let second = TemporaryFile::create(dir.path(), path.file_name().unwrap()).unwrap();
    let (committed, receive) = std::sync::mpsc::channel();

    // Both files are open before either writer commits. The second writer
    // cannot truncate or rename the first writer's inode after its commit.
    std::thread::scope(|scope| {
        let path = &path;
        scope.spawn(move || {
            first.replace(path, &[b'a'; 65_537]).unwrap();
            assert_eq!(std::fs::read(path).unwrap(), vec![b'a'; 65_537]);
            committed.send(()).unwrap();
        });
        scope.spawn(move || {
            receive.recv().unwrap();
            second.replace(path, b"short").unwrap();
        });
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"short");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn concurrent_writes_publish_only_complete_replacements() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let payloads: Vec<_> = (1..=8)
        .map(|index| vec![index; 8191 * usize::from(index)])
        .collect();
    durable_write(&path, &payloads[0]).unwrap();
    let start = std::sync::Barrier::new(payloads.len());
    std::thread::scope(|scope| {
        for bytes in &payloads {
            let path = &path;
            let start = &start;
            let payloads = &payloads;
            scope.spawn(move || {
                let mut observations = Vec::new();
                for _ in 0..4 {
                    start.wait();
                    observations.push((durable_write(path, bytes), std::fs::read(path)));
                }
                for (saved, observed) in observations {
                    saved.unwrap();
                    assert!(
                        payloads.contains(&observed.unwrap()),
                        "a reader observed mixed bytes"
                    );
                }
            });
        }
    });
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn failed_replacement_cleans_only_its_own_temporary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let active = TemporaryFile::create(dir.path(), path.file_name().unwrap()).unwrap();
    let active_path = active.path.clone();
    std::fs::create_dir(&path).unwrap();
    assert!(durable_write(&path, b"cannot replace a directory").is_err());
    assert!(active_path.exists(), "another writer still owns this file");
    std::fs::remove_dir(&path).unwrap();
    active.replace(&path, b"survived").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"survived");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn replacement_preserves_existing_file_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "private settings").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    durable_write(&path, b"updated private settings").unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "updated private settings"
    );
}

#[test]
fn replaces_the_target_and_leaves_no_temporary_behind() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nested").join("state.json");
    durable_write(&path, b"one").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one");
    durable_write(&path, b"two").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
    let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, vec!["state.json".to_string()]);
}

#[test]
fn a_stray_temporary_from_an_interrupted_write_does_not_touch_the_target() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("state.json");
    durable_write(&path, b"committed").unwrap();
    let stray = dir
        .path()
        .join(format!(".state.json.tmp-{}-0", std::process::id()));
    std::fs::write(&stray, b"{\"torn\":").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "committed");
    durable_write(&path, b"next").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "next");
    assert_eq!(
        std::fs::read(&stray).unwrap(),
        b"{\"torn\":",
        "another operation's temporary is never reused or removed"
    );
}

#[test]
fn a_failed_write_keeps_the_previous_content() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("state.json");
    durable_write(&path, b"kept").unwrap();
    // A directory in the target's place: the rename must fail, the file
    // named by `path` no longer exists, and no temporary lingers.
    let blocked = dir.path().join("blocked");
    std::fs::create_dir_all(blocked.join("child")).unwrap();
    assert!(durable_write(&blocked, b"over a directory").is_err());
    assert!(blocked.is_dir());
    assert!(std::fs::read_dir(dir.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".blocked.tmp-")
    }));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
    assert!(durable_write(Path::new(""), b"x").is_err());
}
