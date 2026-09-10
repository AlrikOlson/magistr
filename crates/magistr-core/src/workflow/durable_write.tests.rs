use super::*;

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
        .join(format!(".state.json.tmp-{}", std::process::id()));
    std::fs::write(&stray, b"{\"torn\":").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "committed");
    durable_write(&path, b"next").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "next");
    assert!(
        !stray.exists(),
        "the next write reclaims its temporary name"
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
    assert!(
        !dir.path()
            .join(format!(".blocked.tmp-{}", std::process::id()))
            .exists()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept");
    assert!(durable_write(Path::new(""), b"x").is_err());
}
