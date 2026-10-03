use super::*;

/// Create a temp project with a spec file and git repo.
pub(in crate::workflow) fn setup_project(spec_content: &str) -> (tempfile::TempDir, PathBuf) {
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
    for _ in 0..3 {
        loop_mgr.advance();
    } // Already at max

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
    assert_eq!(loop_mgr.last_status(), IterationStatus::Committed);
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
    loop_mgr.state.last_status = IterationStatus::Committed;

    loop_mgr.persist().unwrap();

    // Verify the file exists at the right path
    let state_path = dir.path().join(".magistr").join("loop-state.json");
    assert!(state_path.exists());

    // Recover and verify state matches
    let recovered = IterationLoop::recover(dir.path(), &spec_path).unwrap();
    assert_eq!(recovered.state(), loop_mgr.state());
}

#[test]
fn recovery_retains_no_commit_stop_before_another_attempt() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    for _ in 0..2 {
        loop_mgr.pre_iteration_check().unwrap();
        assert!(matches!(
            loop_mgr.post_iteration_check(),
            PostCheckResult::NoCommit { .. }
        ));
        loop_mgr.advance();
    }
    assert!(loop_mgr.should_stop_no_commits());
    loop_mgr.persist().unwrap();

    let mut recovered = IterationLoop::recover(dir.path(), &spec_path).unwrap();
    assert_eq!(recovered.state(), loop_mgr.state());
    assert_eq!(recovered.iteration(), 2);
    assert_eq!(recovered.last_status(), IterationStatus::NoCommit);
    assert_eq!(recovered.consecutive_no_commits(), 2);
    assert!(recovered.should_stop_no_commits());
    assert_ne!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    assert_eq!(
        recovered.run_state().terminal_reason,
        Some(StopReason::NoProgress)
    );
}

#[test]
fn repeated_post_check_settles_each_attempt_once_even_after_recovery() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    loop_mgr.pre_iteration_check().unwrap();
    let first = loop_mgr.post_iteration_check();
    assert_eq!(loop_mgr.post_iteration_check(), first);
    assert_eq!(loop_mgr.consecutive_no_commits(), 1);
    loop_mgr.persist().unwrap();

    let mut recovered = IterationLoop::recover(dir.path(), &spec_path).unwrap();
    assert_eq!(recovered.post_iteration_check(), first);
    assert_eq!(recovered.consecutive_no_commits(), 1);
    recovered.advance();
    recovered.pre_iteration_check().unwrap();
    recovered.post_iteration_check();
    assert_eq!(recovered.consecutive_no_commits(), 2);
}

#[test]
fn recovery_refuses_legacy_state_without_known_run_counters() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    std::fs::write(
        loop_state_path(dir.path()),
        r#"{"iteration":2,"max_iterations":8,"last_commit_sha":"abc", "last_status":"no_commit","spec_checked":0,"spec_total":1}"#,
    ).unwrap();

    let error = match IterationLoop::recover(dir.path(), &spec_path) {
        Ok(_) => panic!("unknown no-commit count must not be recovered as zero"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("new run"), "{error}");
}

#[test]
fn recovery_observes_commit_against_the_persisted_attempt_baseline() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    loop_mgr.pre_iteration_check().unwrap();
    loop_mgr.post_iteration_check();
    loop_mgr.advance();
    loop_mgr.pre_iteration_check().unwrap();
    loop_mgr.persist().unwrap();

    let committed = std::process::Command::new("git")
        .args(["commit", "--allow-empty", "-m", "feat: recovered attempt"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(committed.status.success());

    let mut recovered = IterationLoop::recover(dir.path(), &spec_path).unwrap();
    recovered.pre_iteration_check().unwrap();
    assert!(matches!(
        recovered.post_iteration_check(),
        PostCheckResult::CommitMade { .. }
    ));
    assert_eq!(recovered.consecutive_no_commits(), 0);
}

#[test]
fn recovered_run_keeps_mode_budget_limits_and_terminal_reason() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7)
        .unwrap()
        .with_run_mode(RunMode::OneChunk);
    loop_mgr.set_budget_limit(Some(5.0)).unwrap();
    loop_mgr.observe_budget(3.5);
    loop_mgr.stop(StopReason::Cancelled);
    loop_mgr.persist().unwrap();

    let mut recovered = IterationLoop::open_or_recover(dir.path(), &spec_path, 99, None)
        .unwrap()
        .with_run_mode(RunMode::ChunkLoop);
    recovered.observe_budget(0.5);
    recovered.set_budget_limit(None).unwrap();
    assert!(recovered.set_budget_limit(Some(10.0)).is_err());
    assert_eq!(recovered.run_state(), loop_mgr.run_state());
    assert_eq!(recovered.max_iterations(), 7);
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Stopped(StopReason::Cancelled)
    );
}

#[test]
fn explicit_request_after_cancelled_settled_work_starts_fresh_and_archives_it() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut previous = IterationLoop::with_max(dir.path(), &spec_path, 3).unwrap();
    for _ in 0..2 {
        assert_eq!(
            previous.pre_iteration_check().unwrap(),
            PreCheckResult::Continue
        );
        let dispatch = previous
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
            .unwrap()
            .unwrap();
        previous
            .settle_operation(&dispatch, OperationOutcome::Completed)
            .unwrap();
        previous.advance();
    }
    // Match quitting after reserving the next attempt, with all effects settled.
    assert_eq!(
        previous.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    previous.stop(StopReason::Cancelled);
    previous.persist().unwrap();
    let checkpoint = std::fs::read(loop_state_path(dir.path())).unwrap();

    let mut next = IterationLoop::start_or_recover(dir.path(), &spec_path, 8, None).unwrap();
    assert_ne!(next.state().run_id, previous.state().run_id);
    assert_eq!(next.iteration(), 0);
    assert_eq!(next.max_iterations(), 8);
    assert_eq!(next.run_state().terminal_reason, None);
    assert!(next.operations().records().is_empty());
    // Constructor failure or a crash before the caller persists cannot lose the old state.
    assert_eq!(
        std::fs::read(loop_state_path(dir.path())).unwrap(),
        checkpoint
    );
    let archives: Vec<_> = std::fs::read_dir(dir.path().join(".magistr/loop-history"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(archives.len(), 1);
    let archived: IterationState =
        serde_json::from_slice(&std::fs::read(&archives[0]).unwrap()).unwrap();
    assert_eq!(&archived, previous.state());
    assert_eq!(
        next.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    next.persist().unwrap();
    let saved: IterationState =
        serde_json::from_slice(&std::fs::read(loop_state_path(dir.path())).unwrap()).unwrap();
    assert_eq!(saved.run_id, next.state().run_id);
    assert_eq!(
        serde_json::from_slice::<IterationState>(&std::fs::read(&archives[0]).unwrap()).unwrap(),
        archived
    );
}

#[test]
fn explicit_request_can_follow_each_settled_terminal_outcome() {
    for reason in [
        StopReason::IterationLimit,
        StopReason::OneChunkComplete,
        StopReason::SpecComplete,
        StopReason::NoReadyChunk,
        StopReason::NoProgress,
        StopReason::BudgetExceeded,
        StopReason::Aborted,
        StopReason::Failed,
    ] {
        let (dir, spec_path) = setup_project("- [ ] newly available work\n");
        let mut previous = IterationLoop::with_max(dir.path(), &spec_path, 3).unwrap();
        previous.stop(reason);
        previous.persist().unwrap();
        let mut next = IterationLoop::start_or_recover(dir.path(), &spec_path, 5, None).unwrap();
        assert_eq!(next.run_state().terminal_reason, None, "{reason}");
        assert_eq!(next.max_iterations(), 5);
        assert_eq!(
            next.pre_iteration_check().unwrap(),
            PreCheckResult::Continue
        );
    }
}

#[test]
fn explicit_request_preserves_active_and_paused_run_limits() {
    for reason in [None, Some(StopReason::Paused)] {
        let (dir, spec_path) = setup_project("- [ ] unfinished work\n");
        let mut previous = IterationLoop::with_max(dir.path(), &spec_path, 3).unwrap();
        previous.pre_iteration_check().unwrap();
        previous.set_budget_limit(Some(2.0)).unwrap();
        previous.observe_budget(1.0);
        if let Some(reason) = reason {
            previous.stop(reason);
        }
        previous.persist().unwrap();
        let recovered = IterationLoop::start_or_recover(dir.path(), &spec_path, 99, None).unwrap();
        assert_eq!(recovered.state(), previous.state());
        assert!(!dir.path().join(".magistr/loop-history").exists());
    }
}

#[test]
fn explicit_request_refuses_unknown_work_even_when_cancelled() {
    for kind in [
        OperationKind::ProviderDispatch,
        OperationKind::NativeMutation,
    ] {
        let (dir, spec_path) = setup_project("- [ ] unfinished work\n");
        let mut previous = IterationLoop::with_max(dir.path(), &spec_path, 3).unwrap();
        previous.pre_iteration_check().unwrap();
        let operation = previous
            .begin_operation(kind, "work", "unknown")
            .unwrap()
            .unwrap();
        previous.start_operation(&operation, None).unwrap();
        previous.stop(StopReason::Cancelled);
        previous.persist().unwrap();
        let checkpoint = std::fs::read(loop_state_path(dir.path())).unwrap();
        let error = match IterationLoop::start_or_recover(dir.path(), &spec_path, 99, None) {
            Ok(_) => panic!("unknown work cannot authorize another run"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("outcome is not yet known"));
        assert_eq!(
            std::fs::read(loop_state_path(dir.path())).unwrap(),
            checkpoint
        );
        assert!(!dir.path().join(".magistr/loop-history").exists());
    }
}

#[test]
fn explicit_request_keeps_the_checkpoint_if_archiving_fails_or_it_is_invalid() {
    let (dir, spec_path) = setup_project("- [ ] work\n");
    let mut previous = IterationLoop::with_max(dir.path(), &spec_path, 3).unwrap();
    previous.stop(StopReason::Cancelled);
    previous.persist().unwrap();
    let path = loop_state_path(dir.path());
    let checkpoint = std::fs::read(&path).unwrap();
    std::fs::write(
        dir.path().join(".magistr/loop-history"),
        "cannot become a directory",
    )
    .unwrap();
    assert!(IterationLoop::start_or_recover(dir.path(), &spec_path, 5, None).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), checkpoint);
    std::fs::write(&path, "{invalid checkpoint").unwrap();
    assert!(IterationLoop::start_or_recover(dir.path(), &spec_path, 5, None).is_err());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{invalid checkpoint"
    );
}

#[test]
fn budget_exhaustion_survives_recovery_and_prevents_next_attempt() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    loop_mgr.set_budget_limit(Some(2.0)).unwrap();
    loop_mgr.observe_budget(2.0);
    loop_mgr.persist().unwrap();

    let mut recovered = IterationLoop::recover(dir.path(), &spec_path).unwrap();
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::Stopped(StopReason::BudgetExceeded)
    );
    recovered.stop(StopReason::Failed);
    assert_eq!(
        recovered.run_state().terminal_reason,
        Some(StopReason::BudgetExceeded)
    );
    assert_eq!(recovered.iteration(), 0);
    assert_eq!(recovered.state().attempt_started, None);
}

#[test]
fn recovery_refuses_missing_counters_and_inconsistent_legacy_projections() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    let original = serde_json::to_value(loop_mgr.state()).unwrap();
    let mut missing_count = original.clone();
    missing_count["run"]
        .as_object_mut()
        .unwrap()
        .remove("consecutive_no_commits");
    let mut inconsistent_iteration = original.clone();
    inconsistent_iteration["iteration"] = 2.into();
    let mut inconsistent_limit = original.clone();
    inconsistent_limit["max_iterations"] = 70.into();
    let mut missing_attempt = original.clone();
    missing_attempt
        .as_object_mut()
        .unwrap()
        .remove("attempt_started");
    let mut missing_git_baseline = original.clone();
    missing_git_baseline
        .as_object_mut()
        .unwrap()
        .remove("last_commit_sha");
    let mut missing_operations = original.clone();
    missing_operations
        .as_object_mut()
        .unwrap()
        .remove("operations");
    let mut missing_run_id = original;
    missing_run_id.as_object_mut().unwrap().remove("run_id");
    for state in [
        missing_count,
        inconsistent_iteration,
        inconsistent_limit,
        missing_attempt,
        missing_git_baseline,
        missing_operations,
        missing_run_id,
    ] {
        let json = serde_json::to_string(&state).unwrap();
        std::fs::write(loop_state_path(dir.path()), &json).unwrap();
        let error = match IterationLoop::open_or_recover(dir.path(), &spec_path, 99, None) {
            Ok(_) => panic!("invalid state must not silently become a new run"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("new run"), "{error}");
        assert_eq!(
            std::fs::read_to_string(loop_state_path(dir.path())).unwrap(),
            json
        );
    }
}

#[test]
fn run_modes_share_policy_without_applying_chunk_loop_git_guard_to_conversations() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    for mode in [RunMode::Conversation, RunMode::OneChunk] {
        let mut loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7)
            .unwrap()
            .with_run_mode(mode);
        loop_mgr.pre_iteration_check().unwrap();
        loop_mgr.post_iteration_check();
        loop_mgr.advance();
        assert_eq!(loop_mgr.consecutive_no_commits(), 0);
        assert_eq!(
            loop_mgr.pre_iteration_check().unwrap(),
            match mode {
                RunMode::OneChunk => PreCheckResult::Stopped(StopReason::OneChunkComplete),
                _ => PreCheckResult::Continue,
            }
        );
    }
}

#[test]
fn loop_state_keeps_legacy_status_spelling_while_using_domain_status() {
    let (dir, spec_path) = setup_project("- [ ] item 1\n");
    let loop_mgr = IterationLoop::with_max(dir.path(), &spec_path, 7).unwrap();
    for (legacy, canonical) in [
        ("none", IterationStatus::None),
        ("success", IterationStatus::Committed),
        ("no_commit", IterationStatus::NoCommit),
        ("failed", IterationStatus::Failed),
    ] {
        let mut state = serde_json::to_value(loop_mgr.state()).unwrap();
        state["last_status"] = legacy.into();
        let decoded: IterationState = serde_json::from_value(state).unwrap();
        let shared: magistr_domain::workflow::IterationStatus = decoded.last_status;
        assert_eq!(shared, canonical);
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["last_status"],
            legacy
        );
    }
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
        run: RunState {
            completed_attempts: 3,
            ..RunState::new(RunMode::ChunkLoop, 10, None)
        },
        run_id: magistr_domain::identity::RunId::new(
            magistr_domain::identity::ProjectId::new("proj").unwrap(),
            "run-1",
        )
        .unwrap(),
        operations: OperationLedger::new(),
        attempt_started: None,
        native_chunk_id: None,
        native_track: None,
        session_selection: None,
        last_commit_sha: Some("abc1234".to_string()),
        last_status: IterationStatus::Committed,
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
            "run": {
                "mode": "chunk_loop", "completed_attempts": 2, "max_iterations": 8,
                "consecutive_no_commits": 1, "budget_total_usd": 0.0,
                "budget_limit_usd": null, "terminal_reason": null,
                "last_settled_attempt": 1
            },
            "run_id": {"project": "proj", "local": "run-1"},
            "operations": {"records": []},
            "attempt_started": null,
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
        IterationStatus::Committed,
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

/// The loop's own admitted writer on the fixture store.
pub(in crate::workflow) fn native_engines(tmp: &tempfile::TempDir) -> NativeEngines {
    admitted(tmp, magistr_engine::WriterRole::Host)
}

/// The other admitted role on the same store — what the agent's own
/// `magistr mcp-serve` is to the loop.
pub(in crate::workflow) fn agent_engines(tmp: &tempfile::TempDir) -> NativeEngines {
    admitted(tmp, magistr_engine::WriterRole::Service)
}

/// A source that never moves and a project that configures no gates: the
/// evidence capability every writer here is admitted with, so a recorded
/// gate certifies a source and a chunk can complete on it.
struct FixedSource;

impl magistr_engine::native::SourceWitness for FixedSource {
    fn observation(&self) -> magistr_engine::native::SourceObservation {
        magistr_engine::native::SourceObservation {
            workspace_root: "/w".into(),
            workspace_generation: None,
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: "r1".into(),
            source_digest: "r1".into(),
            config_digest: "c".into(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> std::result::Result<magistr_engine::native::SourceFreshness, String> {
        Ok(magistr_engine::native::SourceFreshness::Unchanged)
    }
}

struct FixedEvidence;

impl magistr_engine::native::EvidencePort for FixedEvidence {
    fn observe(
        &self,
    ) -> std::result::Result<Box<dyn magistr_engine::native::SourceWitness>, String> {
        Ok(Box::new(FixedSource))
    }
    fn required_gates(
        &self,
    ) -> std::result::Result<Vec<magistr_engine::native::GateContract>, String> {
        Ok(Vec::new())
    }
}

fn admitted(tmp: &tempfile::TempDir, role: magistr_engine::WriterRole) -> NativeEngines {
    use magistr_engine::{NativeConfig, ProjectAuthority};
    let cfg = NativeConfig::from_lookup(tmp.path(), |_| None)
        .with_data_dir(tmp.path().join("tas-data"))
        .with_evidence(std::sync::Arc::new(FixedEvidence));
    ProjectAuthority::admit_for_project(&cfg, "iteration-native".into(), role)
        .expect("admitted")
        .open()
}

/// Complete `id` the only way a chunk completes: an objective scoped to
/// it, a verification task, a gate recorded green on that task, then
/// `roadmap_complete_chunk` naming the task.
pub(in crate::workflow) fn complete_native_chunk(engines: &NativeEngines, id: &str) {
    engines
        .ship_set_objective(format!("do {id}"), vec![], vec![], format!("chunk:{id}"))
        .expect("objective");
    engines
        .ship_add_task(
            "verify".into(),
            "verify".into(),
            magistr_engine::native::TaskType::Test,
            None,
        )
        .expect("task");
    engines.ship_start_task("verify").expect("active");
    engines
        .ship_record_gate(None, "test", "true".into(), 0, String::new(), true, None)
        .expect("gate");
    engines
        .roadmap_complete_chunk(id, Some("task:verify"))
        .expect("complete on the run's receipts");
}

pub(in crate::workflow) fn add_native_chunk(
    engines: &NativeEngines,
    id: &str,
    priority: u32,
    deps: &[&str],
) {
    use magistr_engine::native::ChunkStatus;
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
fn explicit_new_request_keeps_the_admitted_native_roadmap() {
    let (dir, spec_path) = setup_project("- [x] old file is complete\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "ready", 100, &[]);
    let mut previous =
        IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    previous.stop(StopReason::Cancelled);
    previous.persist().unwrap();
    let mut next =
        IterationLoop::start_or_recover(dir.path(), &spec_path, 5, Some(engines.clone())).unwrap();
    assert!(next.is_native());
    assert_eq!(
        next.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    assert_eq!(next.native().unwrap().roadmap_next().unwrap().id, "ready");
    assert_eq!(
        engines.roadmap_chunk("ready").unwrap().status,
        magistr_engine::native::ChunkStatus::Pending
    );
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
    let writer = agent_engines(&dir);
    writer.roadmap_start_chunk("first").unwrap();
    complete_native_chunk(&writer, "first");

    // The loop sees it without being re-opened and moves to the next chunk.
    let summary = il.spec_summary().unwrap();
    assert_eq!(summary.checked, 1);
    assert_eq!(
        summary.unchecked_items,
        vec!["second — Chunk second".to_string()]
    );

    il.advance();
    writer.roadmap_start_chunk("second").unwrap();
    assert_eq!(il.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    assert_eq!(il.next_native_chunk().unwrap().unwrap().id, "second");
    assert!(
        il.next_prompt(&project_config())
            .unwrap()
            .contains("in progress; resume this work")
    );
    assert_eq!(il.state().spec_checked, 1);
    writer.ship_finalize(vec![], None).unwrap();
    complete_native_chunk(&writer, "second");
    il.advance();
    assert_eq!(
        il.pre_iteration_check().unwrap(),
        PreCheckResult::NoReadyChunk
    );
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
    assert!(prompt.contains(magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE));
    assert!(prompt.contains("Progress: 0/1"), "{prompt}");
    assert!(!prompt.contains("Legacy item"), "{prompt}");
}

#[test]
fn cancelled_native_run_resumes_its_saved_item_and_preserves_the_plan() {
    use magistr_engine::native::TaskType;
    let (dir, spec_path) = setup_project("- [x] Legacy item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "current", 200, &[]);
    let mut first = IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    assert_eq!(
        first.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    engines.roadmap_start_chunk("current").unwrap();
    engines
        .ship_set_objective(
            "Finish the current change".into(),
            vec![],
            vec![],
            "chunk:current".into(),
        )
        .unwrap();
    engines
        .ship_add_task(
            "build".into(),
            "Finish the change".into(),
            TaskType::Implement,
            None,
        )
        .unwrap();
    engines.ship_start_task("build").unwrap();
    let objective = engines.ship_objective();
    let plan = engines.ship_tasks();
    first.stop(StopReason::Cancelled);
    first.persist().unwrap();
    drop(first);

    add_native_chunk(&engines, "urgent", 1, &[]);
    add_native_chunk(&engines, "other", 10, &[]);
    engines.roadmap_start_chunk("other").unwrap();
    let mut resumed =
        IterationLoop::start_or_recover(dir.path(), &spec_path, 5, Some(engines.clone())).unwrap();
    assert_eq!(resumed.max_iterations(), 5);
    assert_eq!(resumed.state().native_chunk_id.as_deref(), Some("current"));
    assert_eq!(
        resumed.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    assert_eq!(resumed.next_native_chunk().unwrap().unwrap().id, "current");
    let prompt = resumed.next_prompt(&project_config()).unwrap();
    assert!(
        prompt.contains("current — Chunk current (in progress; resume this work)"),
        "{prompt}"
    );
    assert!(
        !prompt.contains("Chunk urgent") && !prompt.contains("Chunk other"),
        "{prompt}"
    );
    assert_eq!(engines.ship_objective(), objective);
    assert_eq!(engines.ship_tasks(), plan);
}

#[test]
fn older_native_checkpoint_resumes_the_only_unfinished_item() {
    let (dir, spec_path) = setup_project("- [x] Legacy item\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "unfinished", 100, &[]);
    engines.roadmap_start_chunk("unfinished").unwrap();
    let first = IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    first.persist().unwrap();
    let path = loop_state_path(dir.path());
    let mut old: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("native_chunk_id");
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    drop(first);

    let mut resumed =
        IterationLoop::start_or_recover(dir.path(), &spec_path, 9, Some(engines)).unwrap();
    assert_eq!(resumed.max_iterations(), 3);
    assert_eq!(
        resumed.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    assert_eq!(
        resumed.state().native_chunk_id.as_deref(),
        Some("unfinished")
    );
}

#[test]
fn interrupted_native_item_does_not_bypass_a_live_or_unknown_dispatch() {
    use crate::workflow::runtime_handle::RuntimeHandle;
    for external in [
        None,
        RuntimeHandle::observe(std::process::id()).map(|handle| handle.to_string()),
    ] {
        let (dir, spec_path) = setup_project("- [x] Legacy item\n");
        let engines = native_engines(&dir);
        add_native_chunk(&engines, "unfinished", 100, &[]);
        let mut first =
            IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
        first.pre_iteration_check().unwrap();
        engines.roadmap_start_chunk("unfinished").unwrap();
        let dispatch = first
            .begin_operation(OperationKind::ProviderDispatch, "dispatch", "dispatch")
            .unwrap()
            .unwrap();
        first.start_operation(&dispatch, external).unwrap();
        first.persist().unwrap();
        drop(first);

        let mut resumed =
            IterationLoop::start_or_recover(dir.path(), &spec_path, 3, Some(engines.clone()))
                .unwrap();
        assert!(matches!(
            resumed.pre_iteration_check().unwrap(),
            PreCheckResult::Unreconciled(_)
        ));
        assert_eq!(
            engines.roadmap_chunk("unfinished").unwrap().status,
            magistr_engine::native::ChunkStatus::InProgress
        );
        assert_eq!(resumed.operations().records().len(), 2);
    }
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

#[test]
fn agent_settings_are_persisted_and_cannot_change_during_recovery() {
    let (dir, spec) = setup_project("- [ ] one\n");
    let mut running = IterationLoop::with_max(dir.path(), &spec, 3).unwrap();
    let saved = crate::config::session_selection::SessionSelection {
        agent: "codex".into(),
        model: Some("saved-model".into()),
        effort: Some("high".into()),
        sources: Default::default(),
    };
    running.bind_session_selection(saved.clone()).unwrap();
    running.persist().unwrap();
    let mut recovered = IterationLoop::open_or_recover(dir.path(), &spec, 3, None).unwrap();
    assert_eq!(recovered.state().session_selection.as_ref(), Some(&saved));
    let mut changed = saved.clone();
    changed.model = Some("changed".into());
    assert!(recovered.bind_session_selection(changed).is_err());
    recovered.bind_session_selection(saved.clone()).unwrap();
    assert_eq!(recovered.state().session_selection.as_ref(), Some(&saved));
}

#[test]
fn new_loop_does_not_inherit_a_blocked_item_from_an_ended_run() {
    let (dir, spec_path) = setup_project("- [x] Done\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "previous", 1, &[]);
    add_native_chunk(&engines, "ready", 2, &[]);
    let mut previous =
        IterationLoop::open(dir.path(), &spec_path, 1, Some(engines.clone())).unwrap();
    previous.pre_iteration_check().unwrap();
    engines
        .roadmap_set_status("previous", magistr_engine::native::ChunkStatus::Blocked)
        .unwrap();
    previous.stop(StopReason::Failed);
    previous.persist().unwrap();
    let before = engines.snapshot();

    let mut next =
        IterationLoop::start_or_recover(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    assert_eq!(
        next.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    assert_eq!(next.state().native_chunk_id.as_deref(), Some("ready"));
    assert_eq!(engines.snapshot(), before, "blocked work remains unchanged");
}

#[test]
fn track_loop_keeps_its_scope_after_recovery_and_stops_when_it_is_empty() {
    let (dir, spec_path) = setup_project("- [x] Done\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "chosen", 100, &[]);
    add_native_chunk(&engines, "elsewhere", 1, &[]);
    engines
        .roadmap_set_group("chosen", Some("Surface work".into()))
        .unwrap();
    engines
        .roadmap_set_group("elsewhere", Some("Other work".into()))
        .unwrap();
    let mut first = IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    first.select_track(Some("surface")).unwrap();
    assert_eq!(first.track(), Some("Surface work"));
    assert_eq!(
        first.pre_iteration_check().unwrap(),
        PreCheckResult::Continue
    );
    assert_eq!(first.state().native_chunk_id.as_deref(), Some("chosen"));
    first.persist().unwrap();

    let mut recovered =
        IterationLoop::recover_with(dir.path(), &spec_path, Some(engines.clone())).unwrap();
    recovered.select_track(Some("Surface work")).unwrap();
    let before = recovered.state().clone();
    assert!(recovered.select_track(Some("Other work")).is_err());
    assert_eq!(recovered.state(), &before);
    engines
        .roadmap_set_status("chosen", magistr_engine::native::ChunkStatus::Obsoleted)
        .unwrap();
    assert_eq!(
        recovered.pre_iteration_check().unwrap(),
        PreCheckResult::NoReadyChunk
    );
    assert_eq!(
        engines.roadmap_chunk("elsewhere").unwrap().status,
        magistr_engine::native::ChunkStatus::Pending
    );
}

#[test]
fn track_loop_ignores_unrelated_blocked_and_in_progress_items() {
    let (dir, spec_path) = setup_project("- [x] Done\n");
    let engines = native_engines(&dir);
    for (id, group) in [
        ("ready", "Chosen"),
        ("blocked", "Other"),
        ("unfinished", "Other"),
    ] {
        add_native_chunk(&engines, id, 1, &[]);
        engines.roadmap_set_group(id, Some(group.into())).unwrap();
    }
    engines.roadmap_start_chunk("unfinished").unwrap();
    engines
        .roadmap_set_status("blocked", magistr_engine::native::ChunkStatus::Blocked)
        .unwrap();
    let before = engines.snapshot();
    let mut run = IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    run.select_track(Some("Chosen")).unwrap();
    assert_eq!(run.pre_iteration_check().unwrap(), PreCheckResult::Continue);
    assert_eq!(run.state().native_chunk_id.as_deref(), Some("ready"));
    assert_eq!(engines.snapshot(), before);
}

#[test]
fn track_loop_never_runs_a_dependency_from_another_track_or_a_blocked_pending_item() {
    let (dir, spec_path) = setup_project("- [x] Done\n");
    let engines = native_engines(&dir);
    add_native_chunk(&engines, "dependency", 1, &[]);
    add_native_chunk(&engines, "waiting", 2, &["dependency"]);
    add_native_chunk(&engines, "blocked", 3, &[]);
    for id in ["waiting", "blocked"] {
        engines
            .roadmap_set_group(id, Some("Chosen".into()))
            .unwrap();
    }
    engines
        .roadmap_set_blocked_by(
            "blocked",
            magistr_engine::native::BlockerKind::External,
            "Waiting for input".into(),
            None,
        )
        .unwrap();
    let mut run = IterationLoop::open(dir.path(), &spec_path, 3, Some(engines.clone())).unwrap();
    let before = run.state().clone();
    assert!(run.select_track(Some("Missing")).is_err());
    assert_eq!(run.state(), &before);
    run.select_track(Some("Chosen")).unwrap();
    assert_eq!(
        run.pre_iteration_check().unwrap(),
        PreCheckResult::NoReadyChunk
    );
}
