use super::*;
use magistr_workflow::infra::{PROJECT_DIR, PROJECT_FILE, project_id_for_path, write_project_file};

#[test]
fn declares_the_derived_id_when_nothing_is_declared() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("acme-app");
    std::fs::create_dir_all(&root).expect("mkdir");
    let canonical = root.canonicalize().expect("canonical");

    let declaration = identity_declaration(&root).expect("undeclared project");

    // Identity parity: the same function the server uses, on the same path.
    let json: serde_json::Value = serde_json::from_str(&declaration).expect("json");
    assert_eq!(json["id"], project_id_for_path(&canonical));
    assert!(json.get("name").is_none());
    // Rendering parity: the bytes the workflow itself writes for that id.
    let written = write_project_file(&root, &project_id_for_path(&canonical), None).expect("pin");
    assert_eq!(
        std::fs::read_to_string(written).expect("pinned"),
        declaration
    );
}

#[test]
fn plans_nothing_over_an_existing_declaration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("declared");
    let dir = root.join(PROJECT_DIR);
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join(PROJECT_FILE), "{\"id\":\"example-a1b2c3\"}\n").expect("write");

    assert_eq!(identity_declaration(&root), None);
    let (resolved, source) = resolve_project_id_with(None, Some(&root));
    assert_eq!(resolved, "example-a1b2c3");
    assert_eq!(source, IdSource::RepoFile);

    // An ancestor's declaration is the nested directory's identity too.
    let nested = root.join("crates").join("inner");
    std::fs::create_dir_all(&nested).expect("nested");
    assert_eq!(identity_declaration(&nested), None);
}
