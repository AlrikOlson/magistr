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

// ── Safety invariants ───────────────────────────────────────────────────────

#[test]
fn paths_for_a_standalone_project_stay_in_its_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let dir = root.as_path();
    let paths = [
        magistr_dir(dir),
        project_json(dir).unwrap(),
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
        claude_dir(dir),
        rules_dir(dir),
    ];
    for p in &paths {
        let s = p.to_string_lossy();
        assert!(!s.contains(".."), "path {:?} should not contain '..'", p);
    }
}
