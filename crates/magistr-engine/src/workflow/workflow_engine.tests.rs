use super::*;

fn default_engine() -> WorkflowEngine {
    WorkflowEngine::new(WorkflowConfig::default())
}

fn engine_with_max_iterations(n: u32) -> WorkflowEngine {
    WorkflowEngine::new(WorkflowConfig {
        max_iterations: n,
        ..Default::default()
    })
}

// ── Phase transitions ───────────────────────────────────────────────────────

#[test]
fn starts_at_researching() {
    let e = default_engine();
    assert_eq!(e.phase(), WorkflowPhase::Researching);
}

#[test]
fn forward_transition_succeeds() {
    let mut e = default_engine();
    assert!(e.advance_phase(WorkflowPhase::Planning).is_ok());
    assert_eq!(e.phase(), WorkflowPhase::Planning);
    assert!(e.advance_phase(WorkflowPhase::Executing).is_ok());
    assert!(e.advance_phase(WorkflowPhase::Verifying).is_ok());
    assert!(e.advance_phase(WorkflowPhase::Complete).is_ok());
}

#[test]
fn backward_transition_is_rejected() {
    let mut e = default_engine();
    e.advance_phase(WorkflowPhase::Executing).unwrap();
    let result = e.advance_phase(WorkflowPhase::Researching);
    assert!(result.is_err(), "going backward should fail");
}

#[test]
fn same_phase_transition_is_idempotent() {
    let mut e = default_engine();
    e.advance_phase(WorkflowPhase::Planning).unwrap();
    assert!(e.advance_phase(WorkflowPhase::Planning).is_ok());
    assert_eq!(e.phase(), WorkflowPhase::Planning);
}

#[test]
fn skipping_phases_forward_is_allowed() {
    let mut e = default_engine();
    assert!(
        e.advance_phase(WorkflowPhase::Verifying).is_ok(),
        "skipping from Researching to Verifying should work"
    );
}

#[test]
fn force_phase_bypasses_validation() {
    let mut e = default_engine();
    e.advance_phase(WorkflowPhase::Complete).unwrap();
    e.force_phase(WorkflowPhase::Researching);
    assert_eq!(e.phase(), WorkflowPhase::Researching);
}

// ── Iteration management ────────────────────────────────────────────────────

#[test]
fn advance_iteration_resets_phase() {
    let mut e = default_engine();
    e.advance_phase(WorkflowPhase::Complete).unwrap();
    e.advance_iteration();
    assert_eq!(e.phase(), WorkflowPhase::Researching);
    assert_eq!(e.display_iteration(), 2);
}

#[test]
fn committed_resets_no_commit_counter() {
    let mut e = default_engine();
    e.record_no_commit();
    e.record_no_commit();
    assert_eq!(e.consecutive_no_commits(), 2);
    e.record_committed();
    assert_eq!(e.consecutive_no_commits(), 0);
}

#[test]
fn should_stop_on_iteration_limit() {
    let mut e = engine_with_max_iterations(2);
    assert!(!e.should_stop());
    e.advance_iteration(); // iteration 1
    assert!(!e.should_stop());
    e.advance_iteration(); // iteration 2 = limit
    assert!(e.should_stop());
}

#[test]
fn should_stop_on_repeated_no_commits() {
    let mut e = default_engine();
    for _ in 0..3 {
        e.record_no_commit();
    }
    assert!(
        e.should_stop_no_commits(),
        "3 consecutive no-commits should trigger stop"
    );
}

#[test]
fn should_stop_on_spec_complete() {
    let mut e = default_engine();
    e.update_spec_progress(10, 10);
    assert!(e.is_spec_complete());
    assert!(e.should_stop());
}

#[test]
fn spec_not_complete_when_zero_total() {
    let e = default_engine();
    assert!(!e.is_spec_complete(), "0/0 is not complete");
}

// ── Command validation ──────────────────────────────────────────────────────

#[test]
fn start_iteration_rejected_when_aborted() {
    let mut e = default_engine();
    e.execute_command(&WorkflowCommand::AbortLoop).unwrap();
    let result = e.validate_command(&WorkflowCommand::StartIteration);
    assert!(result.is_err());
}

#[test]
fn pause_succeeds_when_running() {
    let e = default_engine();
    assert!(e.validate_command(&WorkflowCommand::Pause).is_ok());
}

#[test]
fn resume_succeeds_when_paused() {
    let mut e = default_engine();
    e.execute_command(&WorkflowCommand::Pause).unwrap();
    assert!(e.validate_command(&WorkflowCommand::Resume).is_ok());
}

// ── Restore ──────────────────────────────────────────────────

#[test]
fn restore_iteration_puts_the_loop_back_where_a_snapshot_says() {
    let mut engine = default_engine();
    let state = IterationState {
        iteration: 4,
        max_iterations: 10,
        last_status: IterationStatus::NoCommit,
        spec_checked: 3,
        spec_total: 9,
    };
    engine.restore_iteration(state.clone(), 2);

    assert_eq!(engine.iteration(), &state);
    assert_eq!(engine.consecutive_no_commits(), 2);
    assert_eq!(engine.display_iteration(), 5);
}

#[test]
fn a_restored_no_commit_count_still_stops_the_loop() {
    let mut engine = default_engine();
    let state = engine.iteration().clone();
    // Two no-commits on the old machine, one on the new one.
    engine.restore_iteration(state, 2);
    assert!(!engine.should_stop_no_commits());
    engine.record_no_commit();
    assert!(
        engine.should_stop_no_commits(),
        "migrating must not launder the loop's own stop condition"
    );
}
