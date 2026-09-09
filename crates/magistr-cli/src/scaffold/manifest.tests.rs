use super::*;

#[test]
fn content_hash_deterministic() {
    let h1 = content_hash("hello world");
    let h2 = content_hash("hello world");
    assert_eq!(h1, h2);
}

#[test]
fn content_hash_differs_for_different_content() {
    let h1 = content_hash("hello");
    let h2 = content_hash("world");
    assert_ne!(h1, h2);
}

#[test]
fn manifest_load_missing_file_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let manifest = ScaffoldManifest::load(tmp.path());
    assert!(manifest.files.is_empty());
}

#[test]
fn manifest_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".magistr")).unwrap();

    let mut manifest = ScaffoldManifest::new();
    manifest.record("foo.md", "hello");
    manifest.record("bar.md", "world");
    manifest.save(tmp.path()).unwrap();

    let loaded = ScaffoldManifest::load(tmp.path());
    assert_eq!(loaded.files.len(), 2);
    assert_eq!(loaded.files["foo.md"], content_hash("hello"));
    assert_eq!(loaded.files["bar.md"], content_hash("world"));
}

#[test]
fn is_customized_returns_false_when_content_matches() {
    let mut manifest = ScaffoldManifest::new();
    manifest.record("rules.md", "original content");
    assert!(!manifest.is_customized("rules.md", "original content"));
}

#[test]
fn is_customized_returns_true_when_content_differs() {
    let mut manifest = ScaffoldManifest::new();
    manifest.record("rules.md", "original content");
    assert!(manifest.is_customized("rules.md", "user edited this"));
}

#[test]
fn is_customized_returns_true_when_no_entry() {
    let manifest = ScaffoldManifest::new();
    assert!(manifest.is_customized("unknown.md", "anything"));
}

#[test]
fn manifest_load_corrupt_file_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("scaffold-manifest.json"), "not json").unwrap();

    let manifest = ScaffoldManifest::load(tmp.path());
    assert!(manifest.files.is_empty());
}
