use super::*;
use crate::plan::{Phase, PhaseStatus, Task, yaml_path};
use tempfile::TempDir;

fn sample_plan() -> Plan {
    Plan {
        title: "Test Plan".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![Phase {
            id: "E1".into(),
            name: "First Phase".into(),
            tagline: Some("The beginning".into()),
            status: PhaseStatus::ToDo,
            problem: "Things are broken.\n".into(),
            solution: String::new(),
            depends_on: vec![],
            tasks: vec![
                Task {
                    text: "Task A".into(),
                    checked: false,
                    detail: String::new(),
                    subtasks: vec![
                        Task {
                            text: "Subtask A1".into(),
                            checked: true,
                            detail: String::new(),
                            subtasks: vec![],
                        },
                        Task {
                            text: "Subtask A2".into(),
                            checked: false,
                            detail: String::new(),
                            subtasks: vec![],
                        },
                    ],
                },
                Task {
                    text: "Task B".into(),
                    checked: true,
                    detail: "Some detail.\n".into(),
                    subtasks: vec![],
                },
            ],
        }],
        epilogue: String::new(),
    }
}

fn setup() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let yaml = dir.path().join(".magistr-plan.yml");
    (dir, yaml)
}

#[test]
fn plan_exports_disabled_prevents_legacy_auto_export_and_preserves_existing_files() {
    let root = TempDir::new().expect("project");
    std::fs::create_dir(root.path().join(".magistr")).expect("config directory");
    std::fs::write(
        root.path().join(".magistr/project.json"),
        r#"{"plan_exports":false}"#,
    )
    .expect("policy");
    for yaml in [
        root.path().join(".magistr-plan.yml"),
        root.path().join(".magistr/plan.yaml"),
    ] {
        let store = YamlStore::from_plan(&yaml, sample_plan()).expect("store");
        let target = yaml.parent().expect("parent").join("PLAN.md");
        assert!(store.export_markdown().is_err());
        assert!(!target.exists());
        std::fs::write(&target, "preserve existing content").expect("existing file");
        assert!(store.export_markdown().is_err());
        assert_eq!(
            std::fs::read_to_string(&target).expect("file"),
            "preserve existing content"
        );
    }
}

#[test]
fn save_and_load_round_trip() {
    let (_dir, yaml) = setup();
    let plan = sample_plan();

    // Save
    let mut store = YamlStore::from_plan(&yaml, plan).unwrap();
    store.save().unwrap();

    // Load and compare against the store's (sanitized) state
    let expected = store.index().plan().clone();
    let store2 = YamlStore::load(&yaml).unwrap();
    assert_eq!(store2.index().plan(), &expected);
}

#[test]
fn mutation_and_save() {
    let (_dir, yaml) = setup();
    let mut store = YamlStore::from_plan(&yaml, sample_plan()).unwrap();

    // Mutate
    store.index_mut().toggle_task(0, &[0]).unwrap();
    assert!(store.index().is_dirty());

    // Save
    store.save().unwrap();
    assert!(!store.index().is_dirty());

    // Reload and verify
    let store2 = YamlStore::load(&yaml).unwrap();
    assert!(store2.index().tasks(0).unwrap()[0].checked);
}

#[test]
fn reload_picks_up_external_changes() {
    let (_dir, yaml) = setup();
    let mut store = YamlStore::from_plan(&yaml, sample_plan()).unwrap();
    store.save().unwrap();

    // External edit: overwrite the YAML
    let mut modified = sample_plan();
    modified.title = "Externally Modified".into();
    let new_yaml = serde_saphyr::to_string(&modified).unwrap();
    std::fs::write(&yaml, new_yaml).unwrap();

    // Reload
    store.reload().unwrap();
    assert_eq!(store.index().title(), "Externally Modified");
}

#[test]
fn load_invalid_yaml_returns_error() {
    let (_dir, yaml) = setup();
    std::fs::write(&yaml, "not: [valid: yaml: {").unwrap();
    assert!(YamlStore::load(&yaml).is_err());
}

#[test]
fn load_missing_file_returns_error() {
    assert!(YamlStore::load("/nonexistent/path.yaml").is_err());
}

#[test]
fn load_validation_failure_returns_error() {
    let (_dir, yaml) = setup();
    // Empty title should fail validation
    let bad = Plan {
        title: "  ".into(),
        phases: vec![],
        ..sample_plan()
    };
    let bad_yaml = serde_saphyr::to_string(&bad).unwrap();
    std::fs::write(&yaml, bad_yaml).unwrap();
    assert!(YamlStore::load(&yaml).is_err());
}

#[test]
fn atomic_write_creates_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let deep_path = dir.path().join("a").join("b").join("c").join("file.yaml");
    atomic_write(&deep_path, "test").unwrap();
    assert_eq!(std::fs::read_to_string(&deep_path).unwrap(), "test");
}

#[test]
fn atomic_write_no_leftover_tmp() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("file.yaml");
    atomic_write(&path, "content").unwrap();
    assert!(!dir.path().join("file.yaml.tmp").exists());
}

#[test]
fn yaml_path_helper() {
    let p = yaml_path(Path::new("/project"));
    assert_eq!(p, Path::new("/project/.magistr-plan.yml"));
}

#[test]
fn from_plan_validates() {
    let bad = Plan {
        title: "".into(),
        phases: vec![],
        ..sample_plan()
    };
    assert!(YamlStore::from_plan("/tmp/test.yaml", bad).is_err());
}

#[test]
fn save_does_not_generate_markdown() {
    let (dir, yaml) = setup();
    let mut store = YamlStore::from_plan(&yaml, sample_plan()).unwrap();
    store.save().unwrap();

    // save() should NOT create PLAN.md — it only writes YAML
    let md_path = dir.path().join("PLAN.md");
    assert!(!md_path.exists());
}

#[test]
fn export_markdown_creates_file() {
    let (dir, yaml) = setup();
    let mut store = YamlStore::from_plan(&yaml, sample_plan()).unwrap();
    store.save().unwrap();

    let md_path = store.export_markdown().unwrap();
    assert_eq!(md_path, dir.path().join("PLAN.md"));
    assert!(md_path.exists());

    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(content.contains("# Test Plan"));
    assert!(content.contains("- [ ] Task A"));
    assert!(content.contains("  - [x] Subtask A1"));
}

#[test]
fn load_or_create_bootstraps_empty() {
    let dir = TempDir::new().unwrap();
    let yaml_path = dir.path().join(".magistr-plan.yml");
    assert!(!yaml_path.exists());

    let store = YamlStore::load_or_create(&yaml_path, "My Project").unwrap();
    assert_eq!(store.index().title(), "My Project");
    assert_eq!(store.index().stats().total_tasks, 0);
    assert!(yaml_path.exists());
}

#[test]
fn load_or_create_loads_existing() {
    let (_dir, yaml) = setup();
    let mut store = YamlStore::from_plan(&yaml, sample_plan()).unwrap();
    store.save().unwrap();

    let loaded = YamlStore::load_or_create(&yaml, "Ignored Title").unwrap();
    assert_eq!(loaded.index().title(), "Test Plan");
}

#[test]
fn current_yaml_creation_uses_only_the_current_file() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join(".magistr")).unwrap();
    std::fs::write(
        dir.path().join(".magistr/plan.yaml"),
        "invalid unrelated YAML",
    )
    .unwrap();
    std::fs::write(dir.path().join("PLAN.md"), "- [ ] unrelated task").unwrap();
    let path = dir.path().join(".magistr-plan.yml");
    assert!(YamlStore::load(&path).is_err());
    let store = YamlStore::load_or_create(&path, "Current Project").unwrap();
    assert_eq!(store.index().title(), "Current Project");
    assert_eq!(store.index().stats().total_tasks, 0);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("PLAN.md")).unwrap(),
        "- [ ] unrelated task"
    );
}
