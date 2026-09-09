use super::*;
use crate::{
    BudgetConfig, CostEvent, ProjectContext, RoadmapProgress, SessionCommand, SessionStatus,
    WorkflowCommand, WorkflowConfig, WorkflowPhase,
};

fn test_config() -> WorkflowConfig {
    WorkflowConfig {
        project_dir: "/tmp/test".into(),
        spec_path: "ROADMAP.md".into(),
        quality_gate: None,
        max_iterations: 10,
        lifecycle_hooks: vec![],
    }
}

fn test_handle() -> EngineHandle {
    EngineHandle::new("test-session", test_config(), BudgetConfig::none())
}

// ── Construction ────────────────────────────────────────────────────

#[test]
fn new_handle_initial_state() {
    let handle = test_handle();
    assert_eq!(handle.session_id(), "test-session");
    assert_eq!(handle.session_status(), SessionStatus::Created);
    assert_eq!(handle.phase(), WorkflowPhase::Researching);
    assert_eq!(handle.total_cost_usd(), 0.0);
    assert!(!handle.is_budget_exceeded());
    assert!(!handle.is_spec_complete());
}

// ── Clone shares state ──────────────────────────────────────────────

#[test]
fn clone_shares_state() {
    let handle1 = test_handle();
    let handle2 = handle1.clone();

    handle1
        .set_session_status(SessionStatus::InProgress)
        .unwrap();
    assert_eq!(handle2.session_status(), SessionStatus::InProgress);
}

// ── Session commands ────────────────────────────────────────────────

#[test]
fn set_session_status() {
    let handle = test_handle();
    assert!(handle.set_session_status(SessionStatus::InProgress).is_ok());
    assert_eq!(handle.session_status(), SessionStatus::InProgress);
}

#[test]
fn set_session_status_rejects_from_terminal() {
    let handle = test_handle();
    handle.set_session_status(SessionStatus::Completed).unwrap();
    assert!(
        handle
            .set_session_status(SessionStatus::InProgress)
            .is_err()
    );
}

#[test]
fn validate_command() {
    let handle = test_handle();
    // Can't send followup when Created
    let cmd = SessionCommand::SendFollowup {
        content: "hello".into(),
    };
    assert!(handle.validate_command(&cmd).is_err());

    // Can abort anytime
    assert!(handle.validate_command(&SessionCommand::Abort).is_ok());
}

// ── Workflow commands ───────────────────────────────────────────────

#[test]
fn set_phase_advances() {
    let handle = test_handle();
    assert!(handle.set_phase(WorkflowPhase::Planning).is_ok());
    assert_eq!(handle.phase(), WorkflowPhase::Planning);
}

#[test]
fn set_phase_rejects_regression() {
    let handle = test_handle();
    handle.set_phase(WorkflowPhase::Executing).unwrap();
    assert!(handle.set_phase(WorkflowPhase::Planning).is_err());
}

#[test]
fn record_committed_and_advance() {
    let handle = test_handle();
    handle.record_committed();
    handle.advance_iteration();
    assert_eq!(handle.phase(), WorkflowPhase::Researching);
}

// ── Cost commands ───────────────────────────────────────────────────

#[test]
fn record_cost_accumulates() {
    let handle = test_handle();
    handle.record_cost(1.50, 100, 50);
    assert!((handle.total_cost_usd() - 1.50).abs() < f64::EPSILON);

    handle.record_cost(2.00, 200, 100);
    assert!((handle.total_cost_usd() - 3.50).abs() < f64::EPSILON);
}

#[test]
fn budget_exceeded_detection() {
    let handle = EngineHandle::new("budget-test", test_config(), BudgetConfig::with_limit(2.0));
    handle.record_cost(2.50, 100, 50);
    assert!(handle.is_budget_exceeded());
}

#[test]
fn extend_budget() {
    let handle = EngineHandle::new("extend-test", test_config(), BudgetConfig::with_limit(2.0));
    handle.record_cost(2.50, 100, 50);
    assert!(handle.is_budget_exceeded());

    handle.extend_budget(5.0);
    assert!(!handle.is_budget_exceeded());
}

// ── Roadmap commands ────────────────────────────────────────────────

#[test]
fn update_roadmap_progress() {
    let handle = test_handle();
    handle.update_roadmap_progress(RoadmapProgress {
        total_tasks: 10,
        checked_tasks: 5,
        total_phases: 3,
        completed_phases: 1,
    });
    let progress = handle.roadmap_progress();
    assert_eq!(progress.total_tasks, 10);
    assert_eq!(progress.checked_tasks, 5);
    assert!(!handle.is_spec_complete());
}

#[test]
fn spec_complete_detection() {
    let handle = test_handle();
    handle.update_roadmap_progress(RoadmapProgress {
        total_tasks: 10,
        checked_tasks: 10,
        total_phases: 3,
        completed_phases: 3,
    });
    assert!(handle.is_spec_complete());
}

// ── Project context ─────────────────────────────────────────────────

#[test]
fn set_and_get_project_context() {
    let handle = test_handle();
    let ctx = ProjectContext::new(
        vec!["abc1234 feat: stuff".into()],
        20,
        12,
        vec!["src/lib.rs".into()],
        None,
        None,
    );
    handle.set_project_context(ctx.clone());
    assert_eq!(handle.project_context(), ctx);
}

// ── Event subscription ──────────────────────────────────────────────

#[test]
fn event_subscription_receives_events() {
    let handle = test_handle();
    let mut rx = handle.subscribe();

    handle.record_cost(1.0, 100, 50);

    // Should have received at least a CostUpdated event
    let event = rx.try_recv().expect("should receive event");
    assert!(matches!(
        event,
        EngineEvent::Cost(CostEvent::CostUpdated { .. })
    ));
}

#[test]
fn event_subscription_phase_transition() {
    let handle = test_handle();
    let mut rx = handle.subscribe();

    handle.set_phase(WorkflowPhase::Planning).unwrap();

    let event = rx.try_recv().expect("should receive event");
    assert!(matches!(
        event,
        EngineEvent::Workflow(WorkflowEvent::PhaseTransition {
            from: WorkflowPhase::Researching,
            to: WorkflowPhase::Planning,
        })
    ));
}

// ── Workflow commands ────────────────────────────────────────────────

#[test]
fn workflow_command_pause_and_resume() {
    let handle = test_handle();
    assert!(!handle.is_paused());

    handle.workflow_command(WorkflowCommand::Pause).unwrap();
    assert!(handle.is_paused());
    assert!(handle.should_stop());

    handle.workflow_command(WorkflowCommand::Resume).unwrap();
    assert!(!handle.is_paused());
    assert!(!handle.should_stop());
}

#[test]
fn workflow_command_abort() {
    let handle = test_handle();
    handle.workflow_command(WorkflowCommand::AbortLoop).unwrap();
    assert!(handle.should_stop());

    // Cannot start after abort
    assert!(
        handle
            .workflow_command(WorkflowCommand::StartIteration)
            .is_err()
    );
}

#[test]
fn workflow_command_skip_advances() {
    let handle = test_handle();
    let mut rx = handle.subscribe();

    handle.workflow_command(WorkflowCommand::Skip).unwrap();

    let event = rx.try_recv().expect("should receive event");
    assert!(matches!(
        event,
        EngineEvent::Workflow(WorkflowEvent::IterationCompleted {
            committed: false,
            ..
        })
    ));
}

#[test]
fn workflow_command_start_from_complete() {
    let handle = test_handle();
    handle.set_phase(WorkflowPhase::Complete).unwrap();
    handle
        .workflow_command(WorkflowCommand::StartIteration)
        .unwrap();
    assert_eq!(handle.phase(), WorkflowPhase::Researching);
}

#[test]
fn should_stop_respects_limits() {
    let handle = test_handle();
    assert!(!handle.should_stop());

    // Complete the spec
    handle.update_roadmap_progress(RoadmapProgress {
        total_tasks: 10,
        checked_tasks: 10,
        total_phases: 1,
        completed_phases: 1,
    });
    assert!(handle.should_stop());
}

// ── Serialization ───────────────────────────────────────────────────

#[test]
fn engine_event_serialization_roundtrip() {
    let event = EngineEvent::Cost(CostEvent::CostUpdated {
        total_cost_usd: 3.50,
        turn_cost_usd: 1.25,
        turns: 3,
    });
    let json = serde_json::to_string(&event).expect("serialize");
    let parsed: EngineEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, parsed);
}

#[test]
fn workflow_event_serialization_roundtrip() {
    let event = WorkflowEvent::PhaseTransition {
        from: WorkflowPhase::Researching,
        to: WorkflowPhase::Planning,
    };
    let json = serde_json::to_string(&event).expect("serialize");
    let parsed: WorkflowEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, parsed);
}

// ── Persistence and migration ────────────────────────────────

/// A handle standing in for a *second process*: a fresh engine, its own
/// session id, nothing carried over except the document.
fn second_process() -> EngineHandle {
    EngineHandle::new("other-process", test_config(), BudgetConfig::none())
}

/// A first process that has got somewhere worth resuming from.
fn first_process() -> EngineHandle {
    let handle = test_handle();
    handle
        .set_session_status(SessionStatus::InProgress)
        .expect("created -> in progress");
    handle.set_phase(WorkflowPhase::Planning).expect("planning");
    handle
        .set_phase(WorkflowPhase::Executing)
        .expect("executing");
    handle.record_cost(2.5, 900, 300);
    handle.advance_iteration();
    handle.advance_iteration();
    handle.update_roadmap_progress(RoadmapProgress {
        total_tasks: 20,
        checked_tasks: 8,
        total_phases: 4,
        completed_phases: 1,
    });
    handle
}

#[test]
fn a_capture_writes_down_where_the_session_had_got_to() {
    let handle = first_process();
    let snapshot = handle.capture();

    assert_eq!(snapshot.version, crate::SessionSnapshot::VERSION);
    assert_eq!(snapshot.session_id, "test-session");
    assert_eq!(snapshot.status, SessionStatus::InProgress);
    assert_eq!(snapshot.phase, WorkflowPhase::Executing);
    assert_eq!(snapshot.iteration.iteration, 2);
    assert_eq!(snapshot.cost.total_usd, 2.5);
    assert_eq!(snapshot.roadmap.checked_tasks, 8);
    // No native engines here, so nothing to derive a cursor from.
    assert_eq!(snapshot.roadmap_cursor, None);
}

#[test]
fn a_second_process_resumes_the_workflow_where_the_first_stopped() {
    let snapshot = first_process().capture();
    let json = serde_json::to_string(&snapshot).expect("serialize");
    let carried: crate::SessionSnapshot = serde_json::from_str(&json).expect("deserialize");

    let resumed = second_process();
    let restored = resumed.resume(&carried).expect("restores");

    assert_eq!(resumed.session_id(), "test-session");
    assert_eq!(resumed.phase(), WorkflowPhase::Executing);
    assert_eq!(resumed.roadmap_progress().checked_tasks, 8);
    assert_eq!(restored.iteration.iteration, 2);
    // The turn in flight stayed with the process that was running it.
    assert_eq!(resumed.session_status(), SessionStatus::Awaiting);
    assert_eq!(restored.resumed_from, Some(SessionStatus::InProgress));
}

#[test]
fn a_resumed_session_keeps_running_from_where_it_landed() {
    let snapshot = first_process().capture();
    let resumed = second_process();
    resumed.resume(&snapshot).expect("restores");

    // Executing → Verifying is forward from the restored phase, which is
    // only true if the resume actually moved the workflow engine.
    resumed
        .set_phase(WorkflowPhase::Verifying)
        .expect("continues forward from the restored phase");
    resumed.advance_iteration();
    assert_eq!(resumed.phase(), WorkflowPhase::Researching);
}

#[test]
fn a_resume_refuses_a_snapshot_holding_an_unanswered_permission() {
    let snapshot = first_process()
        .capture()
        .holding(crate::SessionResidency::PendingPermission {
            request_id: "req-3".into(),
        });

    let resumed = second_process();
    let err = resumed.resume(&snapshot).expect_err("refused");
    assert!(err.to_string().contains("req-3"));
    // Refused means untouched: the fresh engine is still its own session.
    assert_eq!(resumed.session_id(), "other-process");
    assert_eq!(resumed.phase(), WorkflowPhase::Researching);
}

#[test]
fn a_resume_reports_what_the_move_left_behind() {
    let snapshot = first_process().capture().resident_on(vec![
        crate::SessionResidency::AgentProcess { pid: Some(11) },
        crate::SessionResidency::WorkbenchLock {
            workspace: "/tmp/test".into(),
        },
    ]);
    let restored = second_process().resume(&snapshot).expect("restores");
    assert!(restored.lost_anything());
    assert_eq!(restored.severance_report().len(), 2);
}

#[test]
fn the_no_commit_stop_condition_travels_with_the_snapshot() {
    // The count itself is exercised where `record_no_commit` lives
    // (workflow_engine.tests.rs); what matters here is that the handle
    // carries it in both directions rather than resetting it.
    let mut snapshot = first_process().capture();
    snapshot.consecutive_no_commits = 2;

    let resumed = second_process();
    resumed.resume(&snapshot).expect("restores");
    assert_eq!(resumed.capture().consecutive_no_commits, 2);
}
