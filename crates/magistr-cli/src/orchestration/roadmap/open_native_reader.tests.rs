use super::*;
use magistr_engine::native::{ChunkStatus, ProjectAuthority, WriterRole};

fn write_project_json(dir: &Path, backend: Option<&str>) {
    let magistr = dir.join(".magistr");
    std::fs::create_dir_all(&magistr).expect("mkdir");
    let mut config = serde_json::json!({
        "project": {"name": "settings-test", "type": "cli"},
        "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
    });
    if let Some(b) = backend {
        config["roadmap_backend"] = serde_json::Value::String(b.to_owned());
    }
    std::fs::write(magistr.join("project.json"), config.to_string()).expect("write");
}

#[test]
fn current_backend_resolution_uses_declared_settings_or_native_default() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        roadmap_backend_of(tmp.path()).unwrap(),
        RoadmapBackend::ThinkAndShip
    );
    write_project_json(tmp.path(), None);
    std::fs::write(tmp.path().join(".magistr-roadmap.yml"), "title: t\n").unwrap();
    assert_eq!(
        roadmap_backend_of(tmp.path()).unwrap(),
        RoadmapBackend::ThinkAndShip
    );
    for (backend, expected) in [
        ("yaml", RoadmapBackend::Yaml),
        ("think-and-ship", RoadmapBackend::ThinkAndShip),
    ] {
        write_project_json(tmp.path(), Some(backend));
        assert_eq!(roadmap_backend_of(tmp.path()).unwrap(), expected);
    }
}

#[test]
fn current_native_readers_refuse_invalid_settings() {
    let tmp = tempfile::tempdir().unwrap();
    write_project_json(tmp.path(), Some("invalid"));
    assert!(open_native_reader_with(tmp.path(), |_| None).is_err());
    std::fs::write(tmp.path().join(".git"), "invalid link").unwrap();
    assert!(open_native_reader_with(tmp.path(), |_| None).is_err());
}

#[test]
fn yaml_project_opens_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_project_json(tmp.path(), Some("yaml"));
    assert!(
        open_native_reader_with(tmp.path(), |_| None)
            .unwrap()
            .is_none()
    );
}

#[test]
fn native_project_admits_one_host_and_readers_on_the_configured_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project = tmp.path().join("proj");
    std::fs::create_dir_all(&project).expect("mkdir");
    write_project_json(&project, Some("think-and-ship"));
    assert_eq!(
        roadmap_backend_of(&project).unwrap(),
        RoadmapBackend::ThinkAndShip
    );

    let data_dir = tmp.path().join("data").display().to_string();
    let lookup = move |key: &str| (key == "THINK_AND_SHIP_DATA_DIR").then(|| data_dir.clone());

    let config = magistr_core::evidence::writer_config(&project, &lookup);
    let engines = ProjectAuthority::admit(&config, WriterRole::Service)
        .expect("admitted")
        .open();
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
    let again = open_native_reader_with(&project, &lookup)
        .expect("valid settings")
        .expect("native reader");
    assert_eq!(again.project_id(), engines.project_id());
    assert!(again.roadmap_chunk("one").is_some());
    assert!(
        tmp.path().join("data").exists(),
        "store lives under the injected data dir"
    );
    // A second host on the same project is refused with the lease's reason.
    let refused = ProjectAuthority::admit(&config, WriterRole::Host).expect_err("second host");
    assert!(refused.to_string().contains("project owner"), "{refused}");
    drop(engines);
    ProjectAuthority::admit(&config, WriterRole::Host)
        .expect("the reader does not retain the writer lease");
}
