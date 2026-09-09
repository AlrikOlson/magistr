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
    assert!(!client.engines().project_id().is_empty());
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
