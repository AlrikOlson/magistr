use super::*;

#[test]
fn current_settings_distinguish_absent_and_unreadable_files() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("project.json");
    assert!(read_project_settings(&path).unwrap().is_none());
    std::fs::create_dir(&path).unwrap();
    let error = read_project_settings(&path).unwrap_err().to_string();
    assert!(error.contains(&path.display().to_string()), "{error}");
}

#[test]
fn current_settings_refuse_invalid_backend_values_and_incomplete_documents() {
    for invalid in [
        "{invalid",
        "null",
        "[]",
        "{}",
        r#"{"roadmap_backend":"invalid"}"#,
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("project.json");
        std::fs::write(&path, invalid).unwrap();
        let error = read_project_settings(&path).unwrap_err().to_string();
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
    }
}
#[test]
fn current_settings_parse_defaults_and_refuse_invalid_declared_values() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("project.json");
    let mut config = serde_json::json!({
        "project": {"name": "settings-test", "type": "cli"},
        "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
    });
    std::fs::write(&path, config.to_string()).unwrap();
    let loaded = read_project_settings(&path).unwrap().unwrap();
    assert_eq!(
        loaded.roadmap_backend,
        magistr_engine::RoadmapBackend::ThinkAndShip
    );
    assert!(!loaded.roadmap_exports);

    for value in [
        serde_json::json!("invalid"),
        serde_json::json!(null),
        serde_json::json!(4),
        serde_json::json!({}),
    ] {
        config["roadmap_backend"] = value;
        let original = config.to_string();
        std::fs::write(&path, &original).unwrap();
        assert!(read_project_settings(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
}

#[cfg(unix)]
#[test]
fn current_settings_refuse_a_broken_settings_link() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("project.json");
    std::os::unix::fs::symlink(tmp.path().join("missing.json"), &path).unwrap();
    assert!(read_project_settings(&path).is_err());
}
