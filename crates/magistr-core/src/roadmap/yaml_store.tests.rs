use super::*;
use crate::roadmap::{Phase, PhaseStatus, Task, yaml_path};
use tempfile::TempDir;

fn sample_roadmap() -> Roadmap {
    Roadmap {
        title: "Test Roadmap".into(),
        preamble: String::new(),
        vision: String::new(),
        principles: vec![],
        phases: vec![Phase {
            id: "E1".into(),
            name: "First Phase".into(),
            tagline: Some("The beginning".into()),
            status: PhaseStatus::Pending,
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
    let yaml = dir.path().join(".magistr-roadmap.yml");
    (dir, yaml)
}

#[test]
fn roadmap_exports_disabled_prevents_legacy_auto_export_and_preserves_existing_files() {
    let root = TempDir::new().expect("project");
    std::fs::create_dir(root.path().join(".magistr")).expect("config directory");
    std::fs::write(
        root.path().join(".magistr/project.json"),
        r#"{"roadmap_exports":false}"#,
    )
    .expect("policy");
    for yaml in [
        root.path().join(".magistr-roadmap.yml"),
        root.path().join(".magistr/roadmap.yaml"),
    ] {
        let store = YamlStore::from_roadmap(&yaml, sample_roadmap()).expect("store");
        let target = yaml.parent().expect("parent").join("ROADMAP.md");
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
    let roadmap = sample_roadmap();

    // Save
    let mut store = YamlStore::from_roadmap(&yaml, roadmap).unwrap();
    store.save().unwrap();

    // Load and compare against the store's (sanitized) state
    let expected = store.index().roadmap().clone();
    let store2 = YamlStore::load(&yaml).unwrap();
    assert_eq!(store2.index().roadmap(), &expected);
}

#[test]
fn mutation_and_save() {
    let (_dir, yaml) = setup();
    let mut store = YamlStore::from_roadmap(&yaml, sample_roadmap()).unwrap();

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
    let mut store = YamlStore::from_roadmap(&yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    // External edit: overwrite the YAML
    let mut modified = sample_roadmap();
    modified.title = "Externally Modified".into();
    let new_yaml = serde_yml::to_string(&modified).unwrap();
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
    let bad = Roadmap {
        title: "  ".into(),
        phases: vec![],
        ..sample_roadmap()
    };
    let bad_yaml = serde_yml::to_string(&bad).unwrap();
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
    assert_eq!(p, Path::new("/project/.magistr-roadmap.yml"));
}

#[test]
fn from_roadmap_validates() {
    let bad = Roadmap {
        title: "".into(),
        phases: vec![],
        ..sample_roadmap()
    };
    assert!(YamlStore::from_roadmap("/tmp/test.yaml", bad).is_err());
}

#[test]
fn save_does_not_generate_markdown() {
    let (dir, yaml) = setup();
    let mut store = YamlStore::from_roadmap(&yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    // save() should NOT create ROADMAP.md — it only writes YAML
    let md_path = dir.path().join("ROADMAP.md");
    assert!(!md_path.exists());
}

#[test]
fn export_markdown_creates_file() {
    let (dir, yaml) = setup();
    let mut store = YamlStore::from_roadmap(&yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    let md_path = store.export_markdown().unwrap();
    assert_eq!(md_path, dir.path().join("ROADMAP.md"));
    assert!(md_path.exists());

    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(content.contains("# Test Roadmap"));
    assert!(content.contains("- [ ] Task A"));
    assert!(content.contains("  - [x] Subtask A1"));
}

#[test]
fn load_or_migrate_loads_existing_yaml() {
    let (_dir, yaml) = setup();
    let mut store = YamlStore::from_roadmap(&yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    let loaded = YamlStore::load_or_migrate(&yaml).unwrap();
    assert_eq!(loaded.index().title(), "Test Roadmap");
}

#[test]
fn load_or_migrate_migrates_from_markdown() {
    let dir = TempDir::new().unwrap();
    let md_path = dir.path().join("ROADMAP.md");
    std::fs::write(
        &md_path,
        "# Migrated Project\n\n## E1: Setup\n\n### Tasks\n\n- [ ] First task\n- [x] Done task\n",
    )
    .unwrap();

    let yaml_path = dir.path().join(".magistr-roadmap.yml");
    assert!(!yaml_path.exists());

    let store = YamlStore::load_or_migrate(&yaml_path).unwrap();
    assert_eq!(store.index().title(), "Migrated Project");
    assert_eq!(store.index().stats().total_tasks, 2);
    assert_eq!(store.index().stats().checked_tasks, 1);
    // YAML file should now exist
    assert!(yaml_path.exists());
}

#[test]
fn load_or_migrate_migrates_from_legacy_yaml_path() {
    let dir = TempDir::new().unwrap();
    // Create legacy .magistr/roadmap.yaml
    let legacy_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&legacy_dir).unwrap();
    let legacy_yaml = legacy_dir.join("roadmap.yaml");
    let mut store = YamlStore::from_roadmap(&legacy_yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    // Load from new path — should auto-migrate
    let new_yaml = dir.path().join(".magistr-roadmap.yml");
    assert!(!new_yaml.exists());

    let loaded = YamlStore::load_or_migrate(&new_yaml).unwrap();
    assert_eq!(loaded.index().title(), "Test Roadmap");
    assert!(new_yaml.exists());
}

#[test]
fn load_or_migrate_no_files_returns_error() {
    let dir = TempDir::new().unwrap();
    let yaml_path = dir.path().join(".magistr-roadmap.yml");
    assert!(YamlStore::load_or_migrate(&yaml_path).is_err());
}

#[test]
fn load_or_create_bootstraps_empty() {
    let dir = TempDir::new().unwrap();
    let yaml_path = dir.path().join(".magistr-roadmap.yml");
    assert!(!yaml_path.exists());

    let store = YamlStore::load_or_create(&yaml_path, "My Project").unwrap();
    assert_eq!(store.index().title(), "My Project");
    assert_eq!(store.index().stats().total_tasks, 0);
    assert!(yaml_path.exists());
}

#[test]
fn load_or_create_loads_existing() {
    let (_dir, yaml) = setup();
    let mut store = YamlStore::from_roadmap(&yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    let loaded = YamlStore::load_or_create(&yaml, "Ignored Title").unwrap();
    assert_eq!(loaded.index().title(), "Test Roadmap");
}

#[test]
fn load_or_create_migrates_from_legacy_path() {
    let dir = TempDir::new().unwrap();
    // Create legacy .magistr/roadmap.yaml
    let legacy_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&legacy_dir).unwrap();
    let legacy_yaml = legacy_dir.join("roadmap.yaml");
    let mut store = YamlStore::from_roadmap(&legacy_yaml, sample_roadmap()).unwrap();
    store.save().unwrap();

    // load_or_create from new path — should auto-migrate
    let new_yaml = dir.path().join(".magistr-roadmap.yml");
    let loaded = YamlStore::load_or_create(&new_yaml, "Ignored").unwrap();
    assert_eq!(loaded.index().title(), "Test Roadmap");
    assert!(new_yaml.exists());
}
