use magistr_engine::TasConfig;

use super::*;

fn config(root: &Path, data: &Path) -> TasConfig {
    TasConfig::from_lookup(root, |_| None).with_data_dir(data)
}

#[test]
fn opens_on_a_project_directory_and_creates_the_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let client = EngineClient::with_config(config(&root, &data)).expect("opens");
    assert!(data.is_dir(), "the store directory exists");
    assert!(!client.reader().project_id().is_empty());
    assert!(client.writer().is_some(), "opened as the host");
    assert!(client.socket().to_string_lossy().ends_with(".sock"));
}

#[test]
fn a_missing_root_is_refused_before_anything_else() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("nowhere");
    let err =
        EngineClient::with_config(config(&root, &tmp.path().join("data"))).expect_err("refused");
    assert!(
        matches!(err, EngineError::NoProject(ref p) if *p == root),
        "{err}"
    );
    assert!(err.to_string().contains("not a project directory"));
}

#[test]
fn an_uncreatable_store_is_reported_with_its_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    // A file where the store directory should go.
    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("file");
    let data = blocker.join("data");
    let err = EngineClient::with_config(config(&root, &data)).expect_err("refused");
    assert!(
        matches!(err, EngineError::Store { ref path, .. } if *path == data),
        "{err}"
    );
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn a_non_persistent_store_needs_no_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    EngineClient::with_config(config(&root, &data).persist(false)).expect("opens");
    assert!(!data.exists());
}

#[test]
fn a_second_host_on_the_same_project_is_refused_until_the_first_is_gone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let first = EngineClient::with_config(config(&root, &data)).expect("the host");
    let err = EngineClient::with_config(config(&root, &data)).expect_err("a second host");
    assert!(
        matches!(err, EngineError::Admission { root: ref refused, .. } if *refused == root),
        "{err}"
    );
    assert!(err.to_string().contains("project host owner"), "{err}");
    assert!(std::error::Error::source(&err).is_some());

    // A cockpit beside a loop attaches to the loop's admitted writer rather
    // than admitting a second one; both see the same store and socket.
    let attached = EngineClient::attach(first.writer().expect("the host").clone());
    assert_eq!(attached.socket(), first.socket());
    assert_eq!(attached.reader().project_id(), first.reader().project_id());
    // An observer beside the host needs no lease and holds no writer.
    let viewer = EngineClient::observe_config(config(&root, &data)).expect("an observer");
    assert!(viewer.writer().is_none());
    assert_eq!(viewer.reader().project_id(), first.reader().project_id());
    drop(first);
    // The attached clone still carries the lease.
    EngineClient::with_config(config(&root, &data)).expect_err("still held by the attached clone");
    drop(attached);
    EngineClient::with_config(config(&root, &data)).expect("released with the last writer");
}
