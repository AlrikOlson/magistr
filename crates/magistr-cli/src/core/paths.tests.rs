use super::*;

#[test]
fn magistr_dir_construction() {
    let dir = Path::new("/home/user/project");
    assert_eq!(
        magistr_dir(dir),
        PathBuf::from("/home/user/project/.magistr")
    );
}

#[test]
fn project_json_in_magistr_dir() {
    let dir = Path::new("/home/user/project");
    assert_eq!(
        project_json(dir),
        PathBuf::from("/home/user/project/.magistr/project.json")
    );
}

#[test]
fn plan_in_magistr_dir() {
    let dir = Path::new("/home/user/project");
    assert_eq!(
        plan(dir),
        PathBuf::from("/home/user/project/.magistr/current-plan.md")
    );
}

#[test]
fn task_in_magistr_dir() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        task(dir),
        PathBuf::from("/tmp/proj/.magistr/current-task.md")
    );
}

#[test]
fn memory_in_magistr_dir() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(memory(dir), PathBuf::from("/tmp/proj/.magistr/memory.md"));
}

#[test]
fn iteration_log_in_magistr_dir() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        iteration_log(dir),
        PathBuf::from("/tmp/proj/.magistr/iteration-log.md")
    );
}

#[test]
fn session_state_in_magistr_dir() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        session_state(dir),
        PathBuf::from("/tmp/proj/.magistr/session-state.json")
    );
}

#[test]
fn claude_dir_construction() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(claude_dir(dir), PathBuf::from("/tmp/proj/.claude"));
}

#[test]
fn rules_dir_construction() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(rules_dir(dir), PathBuf::from("/tmp/proj/.claude/rules"));
}

#[test]
fn loop_state_path() {
    assert_eq!(loop_state("myapp"), PathBuf::from("/tmp/myapp-next-loop"));
}

#[test]
fn migrate_jig_dir_renames() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = tmp.path().join(".jig");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("project.json"), "{}").unwrap();

    assert!(migrate_jig_dir(tmp.path()).unwrap());
    assert!(!legacy.exists());
    assert!(tmp.path().join(".magistr").exists());
    assert!(tmp.path().join(".magistr/project.json").exists());
}

#[test]
fn migrate_jig_dir_noop_when_no_legacy() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!migrate_jig_dir(tmp.path()).unwrap());
}

#[test]
fn migrate_jig_dir_noop_when_both_exist() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".jig")).unwrap();
    std::fs::create_dir_all(tmp.path().join(".magistr")).unwrap();
    assert!(!migrate_jig_dir(tmp.path()).unwrap());
}

#[test]
fn has_legacy_jig_dir_detection() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(!has_legacy_jig_dir(tmp.path()));

    std::fs::create_dir_all(tmp.path().join(".jig")).unwrap();
    assert!(has_legacy_jig_dir(tmp.path()));

    // If .magistr already exists, no migration needed
    std::fs::create_dir_all(tmp.path().join(".magistr")).unwrap();
    assert!(!has_legacy_jig_dir(tmp.path()));
}

// ── New path function tests ─────────────────────────────────────────────────

#[test]
fn scaffold_manifest_path() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        scaffold_manifest(dir),
        PathBuf::from("/tmp/proj/.magistr/scaffold-manifest.json")
    );
}

#[test]
fn interview_trace_path() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        interview_trace(dir),
        PathBuf::from("/tmp/proj/.magistr/interview-trace.log")
    );
}

#[test]
fn plugins_dir_path() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        plugins_dir(dir),
        PathBuf::from("/tmp/proj/.magistr/plugins")
    );
}

#[test]
fn roadmap_yaml_at_repo_root() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        roadmap_yaml(dir),
        PathBuf::from("/tmp/proj/.magistr-roadmap.yml")
    );
}

#[test]
fn legacy_roadmap_yaml_in_magistr_dir() {
    let dir = Path::new("/tmp/proj");
    assert_eq!(
        legacy_roadmap_yaml(dir),
        PathBuf::from("/tmp/proj/.magistr/roadmap.yaml")
    );
}

// ── ProjectPaths struct tests ───────────────────────────────────────────────

#[test]
fn project_paths_root() {
    let paths = ProjectPaths::new("/home/user/project");
    assert_eq!(paths.root(), Path::new("/home/user/project"));
}

#[test]
fn project_paths_all_methods() {
    let paths = ProjectPaths::new("/p");
    assert_eq!(paths.magistr_dir(), PathBuf::from("/p/.magistr"));
    assert_eq!(
        paths.project_json(),
        PathBuf::from("/p/.magistr/project.json")
    );
    assert_eq!(paths.plan(), PathBuf::from("/p/.magistr/current-plan.md"));
    assert_eq!(paths.task(), PathBuf::from("/p/.magistr/current-task.md"));
    assert_eq!(
        paths.iteration_log(),
        PathBuf::from("/p/.magistr/iteration-log.md")
    );
    assert_eq!(paths.memory(), PathBuf::from("/p/.magistr/memory.md"));
    assert_eq!(
        paths.session_state(),
        PathBuf::from("/p/.magistr/session-state.json")
    );
    assert_eq!(
        paths.scaffold_manifest(),
        PathBuf::from("/p/.magistr/scaffold-manifest.json")
    );
    assert_eq!(
        paths.interview_trace(),
        PathBuf::from("/p/.magistr/interview-trace.log")
    );
    assert_eq!(paths.plugins_dir(), PathBuf::from("/p/.magistr/plugins"));
    assert_eq!(
        paths.roadmap_yaml(),
        PathBuf::from("/p/.magistr-roadmap.yml")
    );
    assert_eq!(
        paths.legacy_roadmap_yaml(),
        PathBuf::from("/p/.magistr/roadmap.yaml")
    );
    assert_eq!(paths.claude_dir(), PathBuf::from("/p/.claude"));
    assert_eq!(paths.rules_dir(), PathBuf::from("/p/.claude/rules"));
    assert_eq!(
        paths.settings_json(),
        PathBuf::from("/p/.claude/settings.json")
    );
}

#[test]
fn project_paths_from_conversions() {
    let from_pathbuf: ProjectPaths = PathBuf::from("/a").into();
    assert_eq!(from_pathbuf.root(), Path::new("/a"));

    let from_path: ProjectPaths = Path::new("/b").into();
    assert_eq!(from_path.root(), Path::new("/b"));

    let from_str: ProjectPaths = "/c".into();
    assert_eq!(from_str.root(), Path::new("/c"));
}

#[test]
fn project_paths_as_ref() {
    let paths = ProjectPaths::new("/p");
    let r: &Path = paths.as_ref();
    assert_eq!(r, Path::new("/p"));
}

#[test]
fn project_paths_ensure_creates_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::new(tmp.path());
    assert!(!paths.magistr_dir().exists());
    paths.ensure().unwrap();
    assert!(paths.magistr_dir().is_dir());
}

#[test]
fn project_paths_ensure_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = ProjectPaths::new(tmp.path());
    paths.ensure().unwrap();
    paths.ensure().unwrap();
    assert!(paths.magistr_dir().is_dir());
}
