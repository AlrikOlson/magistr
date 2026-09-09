use super::*;

#[test]
fn domain_state_default() {
    let state = DomainState::default();
    assert!(state.files.is_empty());
    assert!(state.agent_plan.is_empty());
    assert!(state.agent_commands.is_empty());
    assert!(state.pending_tools.is_empty());
    assert!(state.active_tasks.is_empty());
    assert!(state.active_file.is_none());
    assert!(state.agent_options.is_none());
    assert!(state.messages.is_empty());
    assert!(state.sync_status.is_empty());
    assert!(!state.roadmap_stale);
}

#[test]
fn domain_state_session_defaults() {
    let state = DomainState::default();
    assert!(state.session.active_tool.is_none());
    assert!(!state.session.thinking_active);
    assert!(state.session.session_id.is_none());
}

#[test]
fn domain_state_workflow_defaults() {
    let state = DomainState::default();
    assert!(state.workflow.workflow_phase.is_none());
    assert!(state.workflow.gate_results.is_empty());
    assert!(state.workflow.loop_progress.is_none());
}

#[test]
fn domain_state_budget_defaults() {
    let state = DomainState::default();
    assert!(state.budget.limit.is_none());
    assert!(state.budget.warn_threshold.is_none());
}
