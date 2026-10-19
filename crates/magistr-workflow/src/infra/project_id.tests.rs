use super::*;

fn declare(root: &Path, id: &str) {
    std::fs::create_dir_all(root.join(PROJECT_DIR)).unwrap();
    std::fs::write(root.join(PROJECT_DIR).join(PROJECT_FILE),
        serde_json::json!({"project":{"id":id,"name":"Example"},"quality_gate":{"test":"cargo test"}}).to_string()).unwrap();
}

#[test]
fn project_settings_supply_the_same_identity_from_nested_directories() {
    let repo = tempfile::tempdir().unwrap();
    let nested = repo.path().join("crates/inner/src");
    std::fs::create_dir_all(&nested).unwrap();
    declare(repo.path(), "shared-project");
    for root in [repo.path(), nested.as_path()] {
        assert_eq!(
            resolve_project_id_with(None, Some(root)),
            ("shared-project".into(), IdSource::RepoFile)
        );
    }
    let declared = declared_identity_in(&nested).unwrap();
    assert_eq!(declared.name.as_deref(), Some("example"));
}

#[test]
fn clones_with_the_same_project_settings_share_identity() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for root in [a.path(), b.path()] {
        declare(root, "shared-project");
    }
    assert_eq!(
        resolve_project_id_with(None, Some(a.path())),
        resolve_project_id_with(None, Some(b.path()))
    );
}

#[test]
fn settings_without_a_pinned_id_use_the_directory_identity() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(PROJECT_DIR)).unwrap();
    std::fs::write(
        root.path().join(PROJECT_DIR).join(PROJECT_FILE),
        r#"{"project":{"name":"Example"}}"#,
    )
    .unwrap();
    let path = root.path().canonicalize().unwrap();
    assert_eq!(
        resolve_project_id_with(None, Some(&path)),
        (project_id_for_path(&path), IdSource::DerivedFromPath)
    );
}

#[test]
fn identity_projection_does_not_expose_connection_settings() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(PROJECT_DIR)).unwrap();
    std::fs::write(root.path().join(PROJECT_DIR).join(PROJECT_FILE),
        r#"{"project":{"id":"example","name":"Example","token":"private"},"cloud_url":"https://example.invalid"}"#).unwrap();
    let identity = declared_identity_in(root.path()).unwrap();
    assert_eq!(
        serde_json::to_value(identity).unwrap(),
        serde_json::json!({"id":"example","name":"example"})
    );
}

#[test]
fn namespace_is_idempotent() {
    assert_eq!(
        namespace_session_id("project", "session"),
        "project__session"
    );
    assert_eq!(
        namespace_session_id("project", "project__session"),
        "project__session"
    );
}

#[test]
fn path_identity_is_deterministic_and_distinguishes_directories() {
    let a = Path::new("/a/example");
    let b = Path::new("/b/example");
    assert_eq!(project_id_for_path(a), project_id_for_path(a));
    assert_ne!(project_id_for_path(a), project_id_for_path(b));
    assert_eq!(path_hash6(a).len(), 6);
}

#[test]
fn sanitizing_limits_identifiers_and_removes_punctuation() {
    assert_eq!(sanitize_project_name(" My Project! "), "my-project");
    assert_eq!(sanitize_project_name(&"a".repeat(64)).len(), 32);
}
