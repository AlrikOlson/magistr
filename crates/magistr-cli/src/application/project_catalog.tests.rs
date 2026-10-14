use super::*;
#[test]
fn an_empty_application_does_not_fabricate_a_project_or_write_history() {
    let temp = tempfile::tempdir().unwrap();
    let inputs =
        ProjectInputs::from_lookup(temp.path(), |_| None).with_data_dir(temp.path().join("data"));
    let projects = recent_projects(&inputs, &[]);
    assert!(projects.is_empty());
    assert!(
        !temp.path().join("data").exists(),
        "discovery never creates history"
    );
}
#[test]
fn resident_projects_are_listed_before_history_settlement() {
    let temp = tempfile::tempdir().unwrap();
    let second = temp.path().join("second");
    std::fs::create_dir(&second).unwrap();
    let inputs = ProjectInputs::from_lookup(temp.path(), |_| None);
    let projects = recent_projects(&inputs, std::slice::from_ref(&second));
    assert_eq!(projects.len(), 1);
    assert_eq!(
        projects[0].directory,
        second.canonicalize().unwrap().display().to_string()
    );
}

#[test]
fn equal_folder_names_keep_their_resolved_project_identities() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("one/app");
    let second = temp.path().join("two/app");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let inputs = ProjectInputs::from_lookup(temp.path(), |_| None);
    let projects = recent_projects(&inputs, &[first.clone(), second.clone()]);
    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].name, projects[1].name);
    assert_ne!(projects[0].id, projects[1].id);
    for (project, path) in projects.iter().zip([first, second]) {
        assert_eq!(
            project.id,
            magistr_core::evidence::writer_config_from_env(path.canonicalize().unwrap())
                .resolve_project_id(),
            "catalog and session admission must resolve the same identity"
        );
    }
}
