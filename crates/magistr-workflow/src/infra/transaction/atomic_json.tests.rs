use super::*;

#[test]
fn failed_replacement_preserves_the_existing_destination_and_leaves_no_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("keep"), "saved").unwrap();
    let failure = write_locked(&path, &vec!["new"]).unwrap_err();
    assert_eq!(
        failure.kind,
        magistr_workflow_contracts::WriteErrorKind::BeforeCommit
    );
    assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "saved");
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn failure_after_replacement_is_uncertain_and_read_back_observes_the_new_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    fs::write(&path, "[1]").unwrap();
    let failure = replace(&path, &vec![2], |_| {
        Err(std::io::Error::other(
            "injected directory synchronization failure",
        ))
    })
    .unwrap_err();
    assert_eq!(
        failure.kind,
        magistr_workflow_contracts::WriteErrorKind::Uncertain
    );
    assert_eq!(failure.effect(), "unknown");
    assert_eq!(
        crate::infra::read_json::<Vec<u32>>(&path).unwrap().value,
        Some(vec![2])
    );
}

#[test]
fn returned_stamp_names_exactly_the_synchronized_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    let first = write_locked(&path, &vec!["first"]).unwrap();
    let second = write_locked(&path, &vec!["a different saved value"]).unwrap();
    let observed = crate::infra::read_json::<Vec<String>>(&path).unwrap();
    assert_eq!(
        observed.source,
        magistr_workflow_contracts::ReadSource::File(second)
    );
    assert_ne!(first, second);
    assert_eq!(observed.value.unwrap(), vec!["a different saved value"]);
}
