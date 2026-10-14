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
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    assert_eq!(
        project_json(&root).unwrap(),
        root.join(".magistr/project.json")
    );
    assert_eq!(
        ProjectPaths::new(&root).project_json().unwrap(),
        project_json(&root).unwrap()
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
fn project_paths_root() {
    let paths = ProjectPaths::new("/home/user/project");
    assert_eq!(paths.root(), Path::new("/home/user/project"));
}

#[test]
fn project_paths_all_methods() {
    let paths = ProjectPaths::new("/p");
    assert_eq!(paths.magistr_dir(), PathBuf::from("/p/.magistr"));
    assert!(paths.project_json().is_err());
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
