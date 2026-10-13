use super::*;
use tempfile::TempDir;

// ── Path construction ───────────────────────────────────────────────────────

#[test]
fn magistr_dir_path() {
    let dir = Path::new("/home/user/project");
    assert_eq!(magistr_dir(dir), dir.join(MAGISTR_DIR));
}

#[test]
fn project_json_path() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    assert_eq!(
        project_json(&root).unwrap(),
        root.join(MAGISTR_DIR).join(PROJECT_JSON)
    );
}

#[test]
fn plan_path() {
    let dir = Path::new("/tmp/test");
    assert_eq!(plan(dir), dir.join(MAGISTR_DIR).join(PLAN_FILE));
}

#[test]
fn claude_dir_path() {
    let dir = Path::new("/tmp/test");
    assert_eq!(claude_dir(dir), dir.join(CLAUDE_DIR));
}

#[test]
fn rules_dir_path() {
    let dir = Path::new("/tmp/test");
    assert_eq!(rules_dir(dir), dir.join(CLAUDE_DIR).join("rules"));
}

#[test]
fn settings_json_path() {
    let dir = Path::new("/tmp/test");
    assert_eq!(
        settings_json(dir),
        dir.join(CLAUDE_DIR).join("settings.json")
    );
}

// ── Migration ───────────────────────────────────────────────────────────────

#[test]
fn migrate_jig_dir_when_no_legacy() {
    let tmp = TempDir::new().unwrap();
    let result = migrate_jig_dir(tmp.path()).unwrap();
    assert!(!result, "should return false when no .jig dir exists");
}

#[test]
fn migrate_jig_dir_when_legacy_exists() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join(LEGACY_JIG_DIR)).unwrap();
    std::fs::write(tmp.path().join(LEGACY_JIG_DIR).join("project.json"), "{}").unwrap();

    let result = migrate_jig_dir(tmp.path()).unwrap();
    assert!(result, "should return true when .jig dir migrated");
    assert!(
        tmp.path().join(MAGISTR_DIR).exists(),
        ".magistr dir should exist after migration"
    );
    assert!(
        !tmp.path().join(LEGACY_JIG_DIR).exists(),
        ".jig dir should be gone after migration"
    );
}

#[test]
fn has_legacy_jig_dir_detection() {
    let tmp = TempDir::new().unwrap();
    assert!(!has_legacy_jig_dir(tmp.path()));

    std::fs::create_dir(tmp.path().join(LEGACY_JIG_DIR)).unwrap();
    assert!(has_legacy_jig_dir(tmp.path()));
}

#[test]
fn loop_state_path() {
    let path = loop_state("my-project");
    assert!(path.to_string_lossy().contains("my-project"));
}

// ── Safety invariants ───────────────────────────────────────────────────────

#[test]
fn paths_for_a_standalone_project_stay_in_its_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let dir = root.as_path();
    let paths = [
        magistr_dir(dir),
        project_json(dir).unwrap(),
        plan(dir),
        task(dir),
        iteration_log(dir),
        memory(dir),
        session_state(dir),
        scaffold_manifest(dir),
        interview_trace(dir),
        plugins_dir(dir),
        roadmap_yaml(dir),
        claude_dir(dir),
        rules_dir(dir),
        settings_json(dir),
    ];
    for p in &paths {
        assert!(
            p.starts_with(dir),
            "path {:?} should be a child of {:?}",
            p,
            dir
        );
    }
}

#[test]
fn no_path_contains_traversal() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let dir = root.as_path();
    let paths = [
        magistr_dir(dir),
        project_json(dir).unwrap(),
        plan(dir),
        claude_dir(dir),
        rules_dir(dir),
    ];
    for p in &paths {
        let s = p.to_string_lossy();
        assert!(!s.contains(".."), "path {:?} should not contain '..'", p);
    }
}

#[test]
fn migrate_does_not_overwrite_existing_magistr_dir() {
    let tmp = TempDir::new().unwrap();
    // Create both .jig and .magistr
    std::fs::create_dir(tmp.path().join(LEGACY_JIG_DIR)).unwrap();
    std::fs::create_dir(tmp.path().join(MAGISTR_DIR)).unwrap();
    std::fs::write(tmp.path().join(MAGISTR_DIR).join("existing.json"), "{}").unwrap();

    // Migration should not overwrite existing .magistr
    let result = migrate_jig_dir(tmp.path()).unwrap();
    assert!(!result, "should not migrate when .magistr already exists");
    assert!(
        tmp.path().join(MAGISTR_DIR).join("existing.json").exists(),
        "existing .magistr content should be preserved"
    );
}
