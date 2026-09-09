use super::*;
use magistr_engine::tas::ChunkStatus;

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
    assert!(open_native_engines_with(tmp.path(), |_| None).is_none());
}

#[test]
fn native_project_opens_engines_on_the_configured_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).expect("mkdir");
    write_project_json(&project, Some("think-and-ship"));
    assert_eq!(roadmap_backend_of(&project), RoadmapBackend::ThinkAndShip);

    let data_dir = tmp.path().join("data").display().to_string();
    let lookup = move |key: &str| (key == "THINK_AND_SHIP_DATA_DIR").then(|| data_dir.clone());

    let engines = open_native_engines_with(&project, &lookup).expect("native engines");
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

    // A second open on the same project + store sees the chunk: the helper
    // resolves the same identity and the same data dir.
    let again = open_native_engines_with(&project, &lookup).expect("native engines again");
    assert_eq!(again.project_id(), engines.project_id());
    assert!(again.roadmap_chunk("one").is_some());
    assert!(
        tmp.path().join("data").exists(),
        "store lives under the injected data dir"
    );
}
