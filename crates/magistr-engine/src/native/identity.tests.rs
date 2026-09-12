use super::*;
use magistr_workflow::infra::{PROJECT_DIR, PROJECT_FILE, project_id_for_path};

#[test]
fn writes_the_derived_id_when_nothing_is_declared() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("acme-app");
    std::fs::create_dir_all(&root).expect("mkdir");
    let canonical = root.canonicalize().expect("canonical");

    let id = ensure_project_file(&root).expect("ensure");

    // Identity parity: the same function the server uses, on the same path.
    assert_eq!(id, project_id_for_path(&canonical));
    let file = root.join(PROJECT_DIR).join(PROJECT_FILE);
    let body = std::fs::read_to_string(&file).expect("project file written");
    let json: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(json["id"], id);
}

#[test]
fn respects_an_existing_declaration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("declared");
    let dir = root.join(PROJECT_DIR);
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join(PROJECT_FILE), "{\"id\":\"example-a1b2c3\"}\n").expect("write");

    let id = ensure_project_file(&root).expect("ensure");
    assert_eq!(id, "example-a1b2c3");
    // A second call is a no-op on the same id.
    assert_eq!(ensure_project_file(&root).expect("again"), "example-a1b2c3");
    let (resolved, source) = resolve_project_id_with(None, Some(&root));
    assert_eq!(resolved, "example-a1b2c3");
    assert_eq!(source, IdSource::RepoFile);
}
