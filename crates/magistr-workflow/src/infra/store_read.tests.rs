use super::*;
use serde_json::{Value, json};

#[test]
fn missing_empty_invalid_and_non_file_are_distinct() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let missing = read_json::<Value>(&path).unwrap();
    assert_eq!(missing.source, ReadSource::Missing);
    assert!(missing.value.is_none());
    fs::write(&path, "[]").unwrap();
    let empty = read_json::<Value>(&path).unwrap();
    assert!(matches!(empty.source, ReadSource::File(_)));
    assert_eq!(empty.value, Some(json!([])));
    fs::write(&path, "{").unwrap();
    assert_eq!(
        read_json::<Value>(&path).unwrap_err().kind,
        ReadErrorKind::Decode
    );
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert_eq!(
        read_json::<Value>(&path).unwrap_err().kind,
        ReadErrorKind::Io
    );
}

#[test]
fn a_replaced_path_never_supplies_the_open_descriptors_stamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let replacement = dir.path().join("replacement.json");
    fs::write(&path, "[1]").unwrap();
    let mut opened = File::open(&path).unwrap();
    fs::write(&replacement, "[2]").unwrap();
    fs::rename(&replacement, &path).unwrap();
    let before = opened.metadata().unwrap();
    let original = read_open_json::<Value>(&mut opened, before).unwrap();
    let current = read_json::<Value>(&path).unwrap();
    assert_eq!(original.value, Some(json!([1])));
    assert_eq!(current.value, Some(json!([2])));
    assert_ne!(original.source, current.source);
}

#[test]
fn an_in_place_change_during_observation_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    fs::write(&path, "[1]").unwrap();
    let mut opened = File::open(&path).unwrap();
    let before = opened.metadata().unwrap();
    fs::write(&path, "[100]").unwrap();
    let error = read_open_json::<Value>(&mut opened, before).unwrap_err();
    assert_eq!(error.kind, ReadErrorKind::Interrupted);
}

#[cfg(unix)]
#[test]
fn a_fifo_is_refused_without_waiting_for_a_writer() {
    use std::os::unix::ffi::OsStrExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: name is a live, NUL-terminated pathname for the duration of the call.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert_eq!(
        read_json::<Value>(&path).unwrap_err().kind,
        ReadErrorKind::Io
    );
}
