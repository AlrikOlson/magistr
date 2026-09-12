use super::*;
use crate::infra::write_project_file;

#[test]
fn captured_override_precedes_the_pin_without_changing_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_project_file(tmp.path(), "pinned-project", Some("Pinned")).expect("pin");
    let pin = tmp.path().join(".think-and-ship/project.json");
    let original = std::fs::read(&pin).expect("original pin");

    assert_eq!(
        resolve_project_id_from(Some(" Captured Project! "), tmp.path()),
        ("captured-project".into(), IdSource::Environment)
    );
    assert_eq!(std::fs::read(pin).expect("unchanged pin"), original);
}

#[test]
fn absent_and_empty_captured_overrides_use_the_nearest_pin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("outer");
    let inner = root.join("inner");
    let nested = inner.join("src");
    std::fs::create_dir_all(&nested).expect("nested project");
    write_project_file(&root, "outer-project", None).expect("outer pin");
    write_project_file(&inner, "inner-project", None).expect("inner pin");

    for captured in [None, Some(""), Some("  "), Some("!!!")] {
        assert_eq!(
            resolve_project_id_from(captured, &nested),
            ("inner-project".into(), IdSource::RepoFile)
        );
    }
}

#[test]
fn unpinned_projects_keep_the_existing_canonical_path_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let supplied = tmp.path().join(".");
    let canonical = supplied.canonicalize().expect("canonical project");
    assert_eq!(
        resolve_project_id_from(None, &supplied),
        (project_id_for_path(&canonical), IdSource::DerivedFromPath)
    );
}

#[test]
fn malformed_pin_keeps_the_existing_path_fallback() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pin_dir = tmp.path().join(".think-and-ship");
    std::fs::create_dir_all(&pin_dir).expect("pin directory");
    let pin = pin_dir.join("project.json");
    std::fs::write(&pin, "{broken").expect("malformed pin");
    let canonical = tmp.path().canonicalize().expect("canonical project");

    assert_eq!(
        resolve_project_id_from(None, tmp.path()),
        (project_id_for_path(&canonical), IdSource::DerivedFromPath)
    );
    assert_eq!(
        std::fs::read_to_string(pin).expect("preserved pin"),
        "{broken"
    );
}
