use super::*;

#[test]
fn plan_path_construction() {
    let dir = Path::new("/home/user/project");
    assert_eq!(
        plan_path(dir),
        PathBuf::from("/home/user/project/.magistr/current-plan.md")
    );
}

#[test]
fn task_path_construction() {
    let dir = Path::new("/home/user/project");
    assert_eq!(
        task_path(dir),
        PathBuf::from("/home/user/project/.magistr/current-task.md")
    );
}

#[test]
fn loop_state_path_construction() {
    assert_eq!(
        loop_state_path("myapp"),
        PathBuf::from("/tmp/myapp-next-loop")
    );
}

#[test]
fn plan_exists_false_for_missing() {
    let dir = tempfile::TempDir::new().unwrap();
    assert!(!plan_exists(dir.path()));
}

#[test]
fn plan_exists_true_when_present() {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("current-plan.md"), "plan content").unwrap();
    assert!(plan_exists(dir.path()));
}

#[test]
fn read_plan_success() {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("current-plan.md"), "my plan").unwrap();
    assert_eq!(read_plan(dir.path()).unwrap(), "my plan");
}

#[test]
fn read_plan_missing() {
    let dir = tempfile::TempDir::new().unwrap();
    assert!(read_plan(dir.path()).is_err());
}

#[test]
fn cleanup_state_removes_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("current-plan.md"), "plan").unwrap();
    std::fs::write(magistr_dir.join("current-task.md"), "task").unwrap();

    cleanup_state(dir.path()).unwrap();

    assert!(!magistr_dir.join("current-plan.md").exists());
    assert!(!magistr_dir.join("current-task.md").exists());
}

#[test]
fn cleanup_state_ok_if_missing() {
    let dir = tempfile::TempDir::new().unwrap();
    // Should not error even if files don't exist
    cleanup_state(dir.path()).unwrap();
}

#[test]
fn loop_state_new() {
    let state = LoopState::new();
    assert_eq!(state.iteration, 0);
    assert_eq!(state.last_status, "none");
}

#[test]
fn loop_state_parse() {
    assert_eq!(
        LoopState::parse("3 success"),
        LoopState {
            iteration: 3,
            last_status: "success".to_string(),
        }
    );
}

#[test]
fn loop_state_parse_with_spaces_in_status() {
    let state = LoopState::parse("2 failed with error");
    assert_eq!(state.iteration, 2);
    assert_eq!(state.last_status, "failed with error");
}

#[test]
fn loop_state_parse_empty() {
    let state = LoopState::parse("");
    assert_eq!(state.iteration, 0);
    assert_eq!(state.last_status, "none");
}

#[test]
fn loop_state_roundtrip() {
    let state = LoopState {
        iteration: 5,
        last_status: "success".to_string(),
    };
    let serialized = state.serialize();
    let parsed = LoopState::parse(&serialized);
    assert_eq!(state, parsed);
}

#[test]
fn loop_state_file_lifecycle() {
    // Use a unique prefix to avoid conflicts with other tests
    let prefix = format!("magistr-test-{}", std::process::id());

    // Clean up in case of previous failed test
    let _ = remove_loop_state(&prefix);

    assert!(!loop_is_active(&prefix));

    let state = LoopState::new();
    write_loop_state(&prefix, &state).unwrap();
    assert!(loop_is_active(&prefix));

    let read = read_loop_state(&prefix).unwrap();
    assert_eq!(read, state);

    remove_loop_state(&prefix).unwrap();
    assert!(!loop_is_active(&prefix));
}
