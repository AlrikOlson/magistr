use super::*;

use crate::workflow::{IterationStatus, WorkflowPhase};

fn restored(severed: Vec<SessionResidency>) -> RestoredSession {
    RestoredSession {
        session: EngineSession::new("s1"),
        iteration: IterationState {
            iteration: 0,
            max_iterations: 10,
            last_status: IterationStatus::None,
            spec_checked: 0,
            spec_total: 0,
        },
        consecutive_no_commits: 0,
        run: RunState::new(crate::workflow::RunMode::ChunkLoop, 10, None),
        cost_governor: CostGovernor::new(crate::BudgetConfig::none()),
        roadmap: RoadmapProgress::default(),
        roadmap_cursor: None,
        severed,
        resumed_from: None,
    }
}

#[test]
fn a_restore_that_lost_nothing_says_so() {
    let report = restored(Vec::new());
    assert!(!report.lost_anything());
    assert!(report.severance_report().is_empty());
}

#[test]
fn every_severed_residency_gets_its_own_line() {
    let report = restored(vec![
        SessionResidency::AgentProcess { pid: Some(7) },
        SessionResidency::Terminal { id: "t1".into() },
    ]);
    assert!(report.lost_anything());
    let lines = report.severance_report();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("agent_process"));
    assert!(lines[1].contains("terminal"));
}

#[test]
fn the_session_carries_the_phase_it_was_restored_with() {
    let mut report = restored(Vec::new());
    report.session.set_phase(WorkflowPhase::Verifying);
    assert_eq!(report.session.phase(), WorkflowPhase::Verifying);
}
