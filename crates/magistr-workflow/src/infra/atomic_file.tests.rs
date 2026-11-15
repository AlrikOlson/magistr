use super::*;

/// A destination deeper than the 260 characters Windows allows a path given
/// to the operating system as written: the depth saved check results reach.
fn deep(root: &Path) -> PathBuf {
    let mut path = root.to_path_buf();
    while path.as_os_str().len() < 300 {
        path.push("a".repeat(64));
    }
    fs::create_dir_all(&path).unwrap();
    path.join("record.json")
}

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn replaces_and_creates_at_a_path_longer_than_the_windows_limit() {
    let directory = tempfile::tempdir().unwrap();
    let path = deep(directory.path());
    AtomicFile::replace(&path, b"first").unwrap();
    AtomicFile::replace(&path, b"second").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");

    let created = path.with_file_name("created.json");
    AtomicFile::create(&created, b"kept").unwrap();
    assert_eq!(fs::read(&created).unwrap(), b"kept");
    assert_eq!(
        names(path.parent().unwrap()),
        ["created.json", "record.json"],
        "no temporary is left beside the saved files"
    );
}

#[test]
fn create_never_replaces_what_is_already_saved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("record.json");
    AtomicFile::create(&path, b"original").unwrap();
    let refused = AtomicFile::create(&path, b"other").unwrap_err();
    assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(names(directory.path()), ["record.json"]);
}

#[test]
fn a_failed_write_leaves_the_destination_and_no_temporary() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("record.json");
    fs::write(&path, b"original").unwrap();
    let failed = AtomicFile::replace_with(&path, |file| {
        file.write_all(b"partial")?;
        Err::<(), _>(io::Error::other("injected"))
    })
    .unwrap_err();
    assert_eq!(failed.to_string(), "injected");
    assert_eq!(fs::read(&path).unwrap(), b"original");
    assert_eq!(names(directory.path()), ["record.json"]);
}

#[test]
fn a_file_written_in_place_replaces_what_an_interrupted_attempt_left() {
    let directory = tempfile::tempdir().unwrap();
    let path = deep(directory.path());
    fs::write(&path, b"a longer half-written record").unwrap();
    AtomicFile::write_in_place(&path, b"whole").unwrap();
    AtomicFile::confirm(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"whole");
    assert_eq!(names(path.parent().unwrap()), ["record.json"]);
}
