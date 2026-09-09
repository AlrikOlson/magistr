use super::*;

/// Create a temp project with a spec file and git repo.
fn setup_project(spec_content: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, spec_content).unwrap();

    // Initialize git repo so HEAD SHA checks work
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();

    std::process::Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    (dir, spec_path)
}

// ── Construction tests ──────────────────────────────────────────────

#[test]
fn new_creates_initial_state() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n- [x] item 2\n- [ ] item 3\n");
    let loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 5).unwrap();

    assert_eq!(loop_mgr.iteration(), 0);
    assert_eq!(loop_mgr.max_iterations(), 5);
    assert_eq!(loop_mgr.display_iteration(), 1);
    assert_eq!(loop_mgr.last_status(), IterationStatus::None);
    assert!(loop_mgr.last_commit_sha().is_some());
    assert_eq!(loop_mgr.state().spec_checked, 1);
    assert_eq!(loop_mgr.state().spec_total, 3);
}

#[test]
fn new_with_missing_spec_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let spec_path = dir.path().join("NONEXISTENT.md");
    let result = IterationLoop::with_max(dir.path(), &spec_path, 5);
    assert!(result.is_err());
}

// ── Pre-iteration check tests ───────────────────────────────────────

#[test]
fn pre_check_continue_when_work_remains() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n- [x] item 2\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();

    assert_eq!(
        loop_mgr.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
}

#[test]
fn pre_check_stops_when_spec_complete() {
    let (dir, spec_path) = setup_project("- [x] item 1\n- [x] item 2\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();

    assert_eq!(
        loop_mgr.pre_iteration_check().unwrap(),
        PreCheckResult::SpecComplete
    );
}

#[test]
fn pre_check_stops_at_max_iterations() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 3).unwrap();
    loop_mgr.state.iteration = 3; // Already at max

    assert_eq!(
        loop_mgr.pre_iteration_check().unwrap(),
        PreCheckResult::MaxIterations
    );
}

#[test]
fn pre_check_updates_spec_counts() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n- [ ] item 2\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();

    // Simulate spec progress by rewriting the file
    std::fs::write(&spec_path, "- [x] item 1\n- [ ] item 2\n").unwrap();
    loop_mgr.pre_iteration_check().unwrap();

    assert_eq!(loop_mgr.state().spec_checked, 1);
    assert_eq!(loop_mgr.state().spec_total, 2);
}

// ── Post-iteration check tests ──────────────────────────────────────

#[test]
fn post_check_detects_new_commit() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();
    loop_mgr.pre_iteration_check().unwrap();

    // Make a commit
    std::fs::write(dir.path().join("new_file.rs"), "// new").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "feat: new file"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let result = loop_mgr.post_iteration_check();
    assert!(matches!(result, PostCheckResult::CommitMade { .. }));
    assert_eq!(loop_mgr.last_status(), IterationStatus::Success);
    assert_eq!(loop_mgr.consecutive_no_commits(), 0);
}

#[test]
fn post_check_warns_when_no_commit() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();
    loop_mgr.pre_iteration_check().unwrap();

    // Don't make a commit
    let result = loop_mgr.post_iteration_check();
    assert!(matches!(result, PostCheckResult::NoCommit { .. }));
    assert_eq!(loop_mgr.last_status(), IterationStatus::NoCommit);
    assert_eq!(loop_mgr.consecutive_no_commits(), 1);
}

#[test]
fn post_check_increments_consecutive_no_commits() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();

    // Two iterations without commits
    loop_mgr.pre_iteration_check().unwrap();
    loop_mgr.post_iteration_check();
    loop_mgr.advance();

    loop_mgr.pre_iteration_check().unwrap();
    loop_mgr.post_iteration_check();
    loop_mgr.advance();

    assert_eq!(loop_mgr.consecutive_no_commits(), 2);
    assert!(loop_mgr.should_stop_no_commits());
}

#[test]
fn post_check_resets_no_commit_counter_on_success() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();

    // First iteration: no commit
    loop_mgr.pre_iteration_check().unwrap();
    loop_mgr.post_iteration_check();
    loop_mgr.advance();
    assert_eq!(loop_mgr.consecutive_no_commits(), 1);

    // Second iteration: make a commit
    loop_mgr.pre_iteration_check().unwrap();
    std::fs::write(dir.path().join("change.rs"), "// change").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "fix: change"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    loop_mgr.post_iteration_check();

    assert_eq!(loop_mgr.consecutive_no_commits(), 0);
    assert!(!loop_mgr.should_stop_no_commits());
}

// ── Advance and mark tests ──────────────────────────────────────────

#[test]
fn advance_increments_iteration() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();

    assert_eq!(loop_mgr.iteration(), 0);
    loop_mgr.advance();
    assert_eq!(loop_mgr.iteration(), 1);
    assert_eq!(loop_mgr.display_iteration(), 2);
}

#[test]
fn mark_failed_sets_status() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 10).unwrap();
    loop_mgr.mark_failed();
    assert_eq!(loop_mgr.last_status(), IterationStatus::Failed);
}

// ── Persistence tests ───────────────────────────────────────────────

#[test]
fn persist_and_recover_roundtrip() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n- [x] item 2\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    loop_mgr.advance();
    loop_mgr.advance();
    loop_mgr.state.last_status = IterationStatus::Success;

    loop_mgr.persist().unwrap();

    // Verify the file exists at the right path
    let state_path = dir.path().join(".magistr").join("loop-state.json");
    assert!(state_path.exists());

    // Recover and verify state matches
    let recovered = IterationLoop::recover(dir.path(), &spec_path).unwrap();
    assert_eq!(recovered.state(), loop_mgr.state());
}

#[test]
fn recover_from_missing_file_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let spec_path = dir.path().join("ROADMAP.md");
    std::fs::write(&spec_path, "- [ ] item\n").unwrap();

    let result = IterationLoop::recover(dir.path(), &spec_path);
    assert!(result.is_err());
}

#[test]
fn cleanup_removes_state_file() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 5).unwrap();

    loop_mgr.persist().unwrap();
    assert!(IterationLoop::has_persisted_state(dir.path()));

    loop_mgr.cleanup().unwrap();
    assert!(!IterationLoop::has_persisted_state(dir.path()));
}

#[test]
fn cleanup_ok_when_no_state_file() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 5).unwrap();
    // Should not error when file doesn't exist
    loop_mgr.cleanup().unwrap();
}

#[test]
fn has_persisted_state_false_initially() {
    let dir = tempfile::TempDir::new().unwrap();
    assert!(!IterationLoop::has_persisted_state(dir.path()));
}

// ── JSON format tests ───────────────────────────────────────────────

#[test]
fn state_serializes_to_json() {
    let state = IterationState {
        iteration: 3,
        max_iterations: 10,
        last_commit_sha: Some("abc1234".to_string()),
        last_status: IterationStatus::Success,
        spec_checked: 5,
        spec_total: 12,
    };

    let json = serde_json::to_string_pretty(&state).unwrap();
    assert!(json.contains("\"iteration\": 3"));
    assert!(json.contains("\"success\""));
    assert!(json.contains("\"abc1234\""));
}

#[test]
fn state_deserializes_from_json() {
    let json = r#"{
            "iteration": 2,
            "max_iterations": 8,
            "last_commit_sha": "def5678",
            "last_status": "no_commit",
            "spec_checked": 3,
            "spec_total": 10
        }"#;

    let state: IterationState = serde_json::from_str(json).unwrap();
    assert_eq!(state.iteration, 2);
    assert_eq!(state.max_iterations, 8);
    assert_eq!(state.last_commit_sha, Some("def5678".to_string()));
    assert_eq!(state.last_status, IterationStatus::NoCommit);
    assert_eq!(state.spec_checked, 3);
    assert_eq!(state.spec_total, 10);
}

#[test]
fn iteration_status_roundtrip() {
    for status in [
        IterationStatus::None,
        IterationStatus::Success,
        IterationStatus::NoCommit,
        IterationStatus::Failed,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let parsed: IterationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }
}

// ── Env var override test ───────────────────────────────────────────

#[test]
fn with_max_ignores_env_var() {
    // Even if MAGISTR_MAX_ITERATIONS is set, with_max uses explicit value
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 42).unwrap();
    assert_eq!(loop_mgr.max_iterations(), 42);
}

// ---- native (think-and-ship) mode ----

fn project_config() -> crate::types::ProjectConfig {
    serde_json::from_value(serde_json::json!({
        "project": {"name": "fixture", "description": "", "type": "cli"},
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "fixture"
    }))
    .expect("project config")
}

fn native_engines(tmp: &tempfile::TempDir) -> NativeEngines {
    use magistr_engine::TasConfig;
    let cfg =
        TasConfig::from_lookup(tmp.path(), |_| None).with_data_dir(tmp.path().join("tas-data"));
    NativeEngines::open_for_project(&cfg, "iteration-native".into())
}

fn add_native_chunk(engines: &NativeEngines, id: &str, priority: u32, deps: &[&str]) {
    use magistr_engine::tas::ChunkStatus;
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|d| (*d).to_string()).collect(),
            None,
            None,
        )
        .expect("add chunk");
}

#[test]
fn native_loop_advances_on_roadmap_next_and_stops_when_nothing_is_ready() {
    // The spec file says "everything done"; the roadmap must win.
    let (dir, spec_path) = setup_project("- [x] Legacy item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "first", 100, &[]);
    add_native_chunk(&engines, "second", 200, &["first"]);

    let mut il = IterationLoop::with_max(dir.path(), &spec_path, 5)
        .unwrap()
        .with_native(Some(engines.clone()));
    assert!(il.is_native());

    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    let summary = il.spec_summary().unwrap();
    assert_eq!(summary.total, 2);
    assert_eq!(summary.checked, 0);
    assert_eq!(
        summary.unchecked_items,
        vec!["first — Chunk first".to_string()]
    );
    assert_eq!(il.state().spec_total, 2);

    // Another process (the agent's mcp-serve) completes the chunk.
    let writer = native_engines(&dir);
    writer.roadmap_start_chunk("first").unwrap();
    writer.roadmap_complete_chunk("first", None).unwrap();

    // The loop sees it without being re-opened and moves to the next chunk.
    let summary = il.spec_summary().unwrap();
    assert_eq!(summary.checked, 1);
    assert_eq!(
        summary.unchecked_items,
        vec!["second — Chunk second".to_string()]
    );

    let writer = native_engines(&dir);
    writer.roadmap_start_chunk("second").unwrap();
    // In progress but not done: nothing is ready, so the loop stops with the
    // native reason — the roadmap is not "complete", one chunk is mid-flight.
    assert_eq!(
        il.pre_iteration_check().unwrap(),
        PreCheckResult::NoReadyChunk
    );
    assert_eq!(il.state().spec_checked, 1);
}

#[test]
fn native_prompt_names_the_next_chunk() {
    let (dir, spec_path) = setup_project("- [ ] Legacy item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "guards", 100, &[]);
    let il = IterationLoop::with_max(dir.path(), &spec_path, 5)
        .unwrap()
        .with_native(Some(engines));
    let prompt = il.next_prompt(&project_config()).unwrap();
    assert!(prompt.contains("guards — Chunk guards"), "{prompt}");
    assert!(prompt.contains("Progress: 0/1"), "{prompt}");
    assert!(!prompt.contains("Legacy item"), "{prompt}");
}

#[test]
fn spec_mode_is_unchanged_without_engines() {
    let (dir, spec_path) = setup_project("- [x] Done\n- [ ] Todo\n");
    let mut il = IterationLoop::with_max(dir.path(), &spec_path, 5).unwrap();
    assert!(!il.is_native());
    assert!(il.native().is_none());
    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    let prompt = il.next_prompt(&project_config()).unwrap();
    assert!(prompt.contains("Todo"), "{prompt}");
}

#[test]
fn open_on_a_native_project_needs_no_spec_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let magistr = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr).unwrap();
    std::fs::write(
        magistr.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "solo", 100, &[]);

    let spec_path = dir.path().join("ROADMAP.md");
    assert!(!spec_path.exists());
    // The canonical constructor takes the engines and never opens the spec file.
    let mut il = IterationLoop::open(dir.path(), &spec_path, 5, Some(engines)).unwrap();
    assert!(il.is_native());
    assert_eq!(il.state().spec_total, 1);
    assert_eq!(il.state().spec_checked, 0);
    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
}
