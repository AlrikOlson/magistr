use super::*;
use magistr_workflow::run::domain::check::{Check, CheckType};
use magistr_workflow::run::domain::goal::VerificationMode;
use magistr_workflow::run::domain::step::{StepStatus, StepType};

fn goal(status: GoalStatus) -> Goal {
    Goal {
        description: "goal".to_owned(),
        acceptance_criteria: Vec::new(),
        constraints: Vec::new(),
        scope: String::new(),
        verification: VerificationMode::Full,
        status,
        project_id: String::new(),
        created_at: None,
        completed_at: None,
    }
}

fn step(id: &str, checks: usize) -> Step {
    Step {
        id: id.to_owned(),
        title: id.to_owned(),
        step_type: StepType::Implement,
        status: StepStatus::Planned,
        estimate: None,
        started_at: None,
        completed_at: None,
        artifacts: Vec::new(),
        checks: (0..checks)
            .map(|_| Check {
                check_result_id: None,
                check_type: CheckType::Test,
                name: "gate".to_owned(),
                passed: false,
                details: String::new(),
                required: true,
                verified: true,
                command: None,
                exit_code: None,
                report: None,
                results: None,
                timestamp: String::new(),
            })
            .collect(),
        progress: Vec::new(),
        waiting_reason: None,
        think_branch: None,
    }
}

#[test]
fn an_empty_run_has_nothing() {
    let s = RunGuardState::from_run(None, &[]);
    assert_eq!(s, RunGuardState::default());
    assert!(!s.has_plan());
    assert!(!s.has_check());
}

#[test]
fn defined_or_active_goal_counts_completed_does_not() {
    let set = |status| RunGuardState::from_run(Some(&goal(status)), &[]).goal_set;
    assert!(set(GoalStatus::Open));
    assert!(set(GoalStatus::Active));
    assert!(!set(GoalStatus::Finished));
    assert!(!set(GoalStatus::Abandoned));
    assert!(!RunGuardState::from_run(None, &[]).goal_set);
}

#[test]
fn plan_needs_goal_and_a_step() {
    let defined = goal(GoalStatus::Open);
    assert!(!RunGuardState::from_run(Some(&defined), &[]).has_plan());
    assert!(!RunGuardState::from_run(None, &[step("t1", 0)]).has_plan());
    let s = RunGuardState::from_run(Some(&defined), &[step("t1", 0), step("t2", 0)]);
    assert!(s.has_plan());
    assert_eq!(s.steps_planned, 2);
}

#[test]
fn checks_are_counted_on_the_current_plans_steps() {
    // The plan is the current cycle's steps: a check from a finished cycle
    // is on no step here and is not counted.
    let active = goal(GoalStatus::Active);
    let s = RunGuardState::from_run(Some(&active), &[step("t1", 1), step("t2", 2)]);
    assert_eq!(s.checks_recorded, 3);
    assert!(s.has_check());
    assert!(!RunGuardState::from_run(Some(&active), &[step("t1", 0)]).has_check());
}

#[test]
fn research_and_plan_are_always_open() {
    let s = RunGuardState::default();
    assert!(s.phase_block("research").is_none());
    assert!(s.phase_block("plan").is_none());
}

#[test]
fn execute_and_verify_need_a_plan() {
    let none = RunGuardState::default();
    let reason = none.phase_block("execute").expect("blocked");
    assert!(reason.contains("run_open"), "{reason}");
    assert!(reason.contains("arrange_steps"), "{reason}");
    assert!(none.phase_block("verify").is_some());

    let planned = RunGuardState {
        goal_set: true,
        steps_planned: 1,
        checks_recorded: 0,
    };
    assert!(planned.phase_block("execute").is_none());
    assert!(planned.phase_block("verify").is_none());
}

#[test]
fn done_needs_a_recorded_check() {
    let planned = RunGuardState {
        goal_set: true,
        steps_planned: 1,
        checks_recorded: 0,
    };
    let reason = planned.phase_block("done").expect("blocked");
    assert!(reason.contains("check_step"), "{reason}");

    let checked = RunGuardState {
        checks_recorded: 1,
        ..planned
    };
    assert!(checked.phase_block("done").is_none());
}

#[test]
fn unknown_phase_is_not_blocked_here() {
    // Phase-name validation belongs to the caller; the guard only rules on
    // the phases it knows.
    assert!(RunGuardState::default().phase_block("bogus").is_none());
}
