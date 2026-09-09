use super::*;

#[test]
fn workflow_state_default() {
    let state = WorkflowState::default();
    assert!(state.workflow_phase.is_none());
    assert!(!state.plan_file_written);
    assert!(state.gate_results.is_empty());
    assert!(state.gate_results_history.is_empty());
    assert_eq!(state.consecutive_failures, 0);
    assert!(state.loop_progress.is_none());
}

#[test]
fn set_workflow_phase_initial() {
    let mut state = WorkflowState::default();
    let msg = state.set_workflow_phase(WorkflowPhase::Researching);
    assert!(msg.is_none()); // First set, no transition message
    assert_eq!(state.workflow_phase, Some(WorkflowPhase::Researching));
}

#[test]
fn set_workflow_phase_transition() {
    let mut state = WorkflowState::default();
    state.set_workflow_phase(WorkflowPhase::Researching);
    let msg = state.set_workflow_phase(WorkflowPhase::Planning);
    assert_eq!(msg, Some("Phase: Planning...".into()));
    assert_eq!(state.workflow_phase, Some(WorkflowPhase::Planning));
}

#[test]
fn set_workflow_phase_same_no_message() {
    let mut state = WorkflowState::default();
    state.set_workflow_phase(WorkflowPhase::Executing);
    let msg = state.set_workflow_phase(WorkflowPhase::Executing);
    assert!(msg.is_none());
}

#[test]
fn completed_phase_message_has_no_progress_punctuation() {
    let mut state = WorkflowState::default();
    state.set_workflow_phase(WorkflowPhase::Verifying);
    assert_eq!(
        state.set_workflow_phase(WorkflowPhase::Complete),
        Some("Phase: Complete".into())
    );
}

#[test]
fn push_gate_history() {
    let mut state = WorkflowState::default();
    // Empty results should not push.
    state.push_gate_history();
    assert!(state.gate_results_history.is_empty());

    // Non-empty results push.
    state.gate_results.push(GateResult {
        name: "test".into(),
        passed: true,
        duration_secs: 1.0,
        output: None,
        test_count: None,
        rows: Vec::new(),
    });
    state.push_gate_history();
    assert_eq!(state.gate_results_history.len(), 1);
}

#[test]
fn push_gate_history_caps_at_max() {
    let mut state = WorkflowState::default();
    for i in 0..GATE_HISTORY_MAX + 5 {
        state.gate_results = vec![GateResult {
            name: format!("run{i}"),
            passed: true,
            duration_secs: 1.0,
            output: None,
            test_count: None,
            rows: Vec::new(),
        }];
        state.push_gate_history();
    }
    assert_eq!(state.gate_results_history.len(), GATE_HISTORY_MAX);
    // Oldest should have been dropped — first remaining is run5.
    assert_eq!(state.gate_results_history[0][0].name, "run5");
}
