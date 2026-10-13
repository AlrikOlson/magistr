use super::*;
#[test]
fn an_open_project_is_listed_without_creating_a_second_registry() {
    let temp = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(temp.path(), |_| None);
    let projects = recent_projects(temp.path(), &inputs, &[]);
    assert_eq!(projects.len(), 1);
    assert_eq!(
        projects[0].directory,
        temp.path().canonicalize().unwrap().display().to_string()
    );
}
#[test]
fn resident_projects_are_listed_before_history_settlement() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let inputs = ProjectInputs::from_lookup(&first, |_| None);
    let projects = recent_projects(&first, &inputs, std::slice::from_ref(&second));
    assert_eq!(projects.len(), 2);
    assert!(
        projects
            .iter()
            .any(|p| p.directory == second.canonicalize().unwrap().display().to_string())
    );
    assert!(
        !temp.path().join("sessions.db").exists(),
        "discovery does not write history"
    );
}
