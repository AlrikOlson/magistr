use super::*;

fn project(dir: &Path, json: &str) {
    let magistr = dir.join(".magistr");
    std::fs::create_dir_all(&magistr).unwrap();
    std::fs::write(magistr.join("project.json"), json).unwrap();
}

#[test]
fn a_declared_backend_wins() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), r#"{"roadmap_backend":"yaml"}"#);
    std::fs::write(tmp.path().join(".magistr-roadmap.yml"), "title: x\n").unwrap();
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::Yaml);

    project(tmp.path(), r#"{"roadmap_backend":"think-and-ship"}"#);
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::ThinkAndShip);
    assert_eq!(
        declared_backend(tmp.path()),
        Some(RoadmapBackend::ThinkAndShip)
    );
}

#[test]
fn no_project_json_means_a_spec_file_project() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(declared_backend(tmp.path()), None);
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::Yaml);
}

#[test]
fn a_fresh_project_without_the_key_is_native() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), r#"{"spec":{"file":".magistr-roadmap.yml"}}"#);
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::ThinkAndShip);
}

#[test]
fn a_legacy_project_with_a_yaml_roadmap_stays_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), r#"{"spec":{"file":"ROADMAP.md"}}"#);
    std::fs::write(tmp.path().join(".magistr-roadmap.yml"), "title: x\n").unwrap();
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::Yaml);
}

#[test]
fn a_legacy_project_with_the_old_yaml_path_stays_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), "{}");
    std::fs::write(tmp.path().join(".magistr/roadmap.yaml"), "title: x\n").unwrap();
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::Yaml);
}

#[test]
fn a_legacy_project_whose_spec_file_exists_stays_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), r#"{"spec":{"file":"ROADMAP.md"}}"#);
    std::fs::write(tmp.path().join("ROADMAP.md"), "- [ ] one\n").unwrap();
    assert_eq!(backend_for(tmp.path()), RoadmapBackend::Yaml);
}

#[test]
fn declared_value_short_circuits_the_inference() {
    let tmp = tempfile::tempdir().unwrap();
    project(tmp.path(), "{}");
    std::fs::write(tmp.path().join(".magistr-roadmap.yml"), "title: x\n").unwrap();
    assert_eq!(
        backend_for_declared(tmp.path(), Some(RoadmapBackend::ThinkAndShip)),
        RoadmapBackend::ThinkAndShip
    );
    assert_eq!(backend_for_declared(tmp.path(), None), RoadmapBackend::Yaml);
}
