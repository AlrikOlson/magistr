use super::super::admit_native_host::admit_native_host_with;
use super::*;
use magistr_engine::native::ChunkStatus;

fn write_project_json(dir: &Path, backend: Option<&str>) {
    let magistr = dir.join(".magistr");
    std::fs::create_dir_all(&magistr).expect("mkdir");
    let mut config = serde_json::json!({
        "project": {"name": "t", "description": "", "type": "cli"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
    });
    if let Some(b) = backend {
        config["roadmap_backend"] = serde_json::Value::String(b.to_owned());
    }
    std::fs::write(magistr.join("project.json"), config.to_string()).expect("write");
}

#[test]
fn backend_resolution_follows_backend_for() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // No project.json: a spec-file project, yaml.
    assert_eq!(roadmap_backend_of(tmp.path()), RoadmapBackend::Yaml);
    // A fresh project.json without the key: the think-and-ship default.
    write_project_json(tmp.path(), None);
    assert_eq!(roadmap_backend_of(tmp.path()), RoadmapBackend::ThinkAndShip);
    // …unless a YAML roadmap already exists (a project that predates the key).
    std::fs::write(tmp.path().join(".magistr-roadmap.yml"), "title: t\n").expect("yaml");
    assert_eq!(roadmap_backend_of(tmp.path()), RoadmapBackend::Yaml);
    // A declared value always wins.
    write_project_json(tmp.path(), Some("yaml"));
    assert_eq!(roadmap_backend_of(tmp.path()), RoadmapBackend::Yaml);
    write_project_json(tmp.path(), Some("think-and-ship"));
    assert_eq!(roadmap_backend_of(tmp.path()), RoadmapBackend::ThinkAndShip);
}

#[test]
fn yaml_project_opens_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_project_json(tmp.path(), Some("yaml"));
    assert!(open_native_reader_with(tmp.path(), |_| None).is_none());
    assert!(
        admit_native_host_with(tmp.path(), |_| None)
            .expect("nothing to admit")
            .is_none()
    );
}

#[test]
fn native_project_admits_one_host_and_readers_on_the_configured_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).expect("mkdir");
    write_project_json(&project, Some("think-and-ship"));
    assert_eq!(roadmap_backend_of(&project), RoadmapBackend::ThinkAndShip);

    let data_dir = tmp.path().join("data").display().to_string();
    let lookup = move |key: &str| (key == "THINK_AND_SHIP_DATA_DIR").then(|| data_dir.clone());

    let engines = admit_native_host_with(&project, &lookup)
        .expect("admitted")
        .expect("native engines");
    engines
        .roadmap_add_chunk(
            "one".into(),
            "One".into(),
            String::new(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("add");

    // A reader on the same project + store sees the chunk without a lease:
    // the helper resolves the same identity and the same data dir.
    let again = open_native_reader_with(&project, &lookup).expect("native reader");
    assert_eq!(again.project_id(), engines.project_id());
    assert!(again.roadmap_chunk("one").is_some());
    assert!(
        tmp.path().join("data").exists(),
        "store lives under the injected data dir"
    );
    // A second host on the same project is refused with the lease's reason.
    let refused = admit_native_host_with(&project, &lookup).expect_err("second host");
    assert!(
        refused.to_string().contains("project host owner"),
        "{refused}"
    );
    drop(engines);
    assert!(
        admit_native_host_with(&project, &lookup)
            .expect("released")
            .is_some()
    );
}
