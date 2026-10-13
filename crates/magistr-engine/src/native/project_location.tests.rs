use super::*;
#[test]
fn aliases_resolve_to_one_directory_and_missing_paths_refuse() {
    let temp = tempfile::tempdir().expect("temporary");
    let location = ProjectLocation::resolve(&temp.path().join(".")).expect("location");
    assert_eq!(
        location.working_copy,
        temp.path().canonicalize().expect("canonical")
    );
    assert_eq!(location.authority_root, location.working_copy);
    assert!(ProjectLocation::resolve(&temp.path().join("missing")).is_err());
}
