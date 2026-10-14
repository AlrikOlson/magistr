use super::*;

fn project(dir: &Path, backend: Option<&str>, spec: Option<&str>) {
    let mut config = serde_json::json!({
        "project": {"name": "settings-test", "type": "cli"},
        "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
    });
    if let Some(backend) = backend {
        config["roadmap_backend"] = backend.into();
    }
    if let Some(spec) = spec {
        config["spec"] = serde_json::json!({"file": spec, "unchecked_pattern": "- [ ]"});
    }
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::write(dir.join(".magistr/project.json"), config.to_string()).unwrap();
}

#[test]
fn current_backend_declarations_are_authoritative() {
    let tmp = tempfile::tempdir().unwrap();
    for (declared, expected) in [
        ("yaml", RoadmapBackend::Yaml),
        ("think-and-ship", RoadmapBackend::ThinkAndShip),
    ] {
        project(tmp.path(), Some(declared), None);
        assert_eq!(backend_for(tmp.path()).unwrap(), expected);
    }
}

#[test]
fn current_backend_without_project_settings_uses_native_default() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        backend_for(tmp.path()).unwrap(),
        RoadmapBackend::ThinkAndShip
    );
}

#[test]
fn current_backend_default_does_not_depend_on_surrounding_files() {
    for file in [
        ".magistr-roadmap.yml",
        ".magistr/roadmap.yaml",
        "ROADMAP.md",
        "requirements.md",
    ] {
        let tmp = tempfile::tempdir().unwrap();
        project(tmp.path(), None, Some(file));
        std::fs::write(tmp.path().join(file), "unrelated file").unwrap();
        assert_eq!(
            backend_for(tmp.path()).unwrap(),
            RoadmapBackend::ThinkAndShip,
            "{file}"
        );
        assert_eq!(
            crate::validate::load_project_config(tmp.path())
                .unwrap()
                .roadmap_backend,
            RoadmapBackend::ThinkAndShip,
            "{file}"
        );
    }
}
#[test]
fn current_backend_refuses_invalid_settings_and_git_links() {
    for invalid in ["{invalid", "null", "{}", r#"{"roadmap_backend":"unknown"}"#] {
        let tmp = tempfile::tempdir().unwrap();
        project(tmp.path(), None, None);
        let path = crate::paths::project_json(tmp.path()).unwrap();
        std::fs::write(&path, invalid).unwrap();
        let error = backend_for(tmp.path()).unwrap_err().to_string();
        assert!(error.contains(&path.display().to_string()), "{error}");
    }
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join(".git"), "not a git link").unwrap();
    assert!(backend_for(tmp.path()).is_err());
}
#[test]
fn current_settings_are_shared_by_linked_working_copies() {
    use crate::config::inputs::{InputOrigin, ProjectInputs};

    let tmp = tempfile::tempdir().unwrap();
    let main = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    let gitdir = main.join(".git/worktrees/linked");
    std::fs::create_dir_all(&gitdir).unwrap();
    std::fs::create_dir_all(&linked).unwrap();
    std::fs::write(
        linked.join(".git"),
        format!("gitdir: {}\n", gitdir.display()),
    )
    .unwrap();
    std::fs::write(gitdir.join("commondir"), "../..\n").unwrap();
    std::fs::write(
        gitdir.join("gitdir"),
        format!("{}\n", linked.join(".git").display()),
    )
    .unwrap();

    project(&main, Some("yaml"), None);
    project(&linked, Some("think-and-ship"), None);
    let shared = crate::paths::project_json(&main).unwrap();
    assert_eq!(crate::paths::project_json(&linked).unwrap(), shared);
    let inputs = ProjectInputs::from_lookup(&linked, |_| None);
    for backend in ["yaml", "think-and-ship"] {
        project(&main, Some(backend), None);
        let original = std::fs::read_to_string(&shared).unwrap();
        let declared = crate::validate::load_project_config(&main).unwrap();
        assert_eq!(
            crate::validate::load_project_config(&linked).unwrap(),
            declared
        );
        assert_eq!(backend_for(&linked).unwrap(), declared.roadmap_backend);
        let captured = inputs.project_config().unwrap();
        assert_eq!(captured.value, Some(declared));
        assert_eq!(captured.origin, InputOrigin::Loaded(shared.clone()));
        assert_eq!(std::fs::read_to_string(&shared).unwrap(), original);
    }

    std::fs::write(&shared, "{invalid shared settings").unwrap();
    assert!(backend_for(&linked).is_err());
    assert!(inputs.project_config().is_err());
    assert!(crate::validate::load_project_config(&linked).is_err());

    std::fs::write(
        gitdir.join("gitdir"),
        main.join(".git").display().to_string(),
    )
    .unwrap();
    assert!(backend_for(&linked).is_err());
}
