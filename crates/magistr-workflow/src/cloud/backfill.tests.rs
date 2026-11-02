use super::*;
use crate::cloud::envelope::Kind;
use crate::plan::domain::{Task, TaskStatus};

/// A plan with the supplied tasks and no tracker state.
fn plan_of(tasks: Vec<Task>) -> Plan {
    Plan {
        project_id: "t".into(),
        tasks,
        ..Plan::default()
    }
}
use crate::run::domain::goal::{GoalStatus, VerificationMode};
use crate::run::domain::progress::{Progress, ProgressType};
use crate::run::domain::step::{StepStatus, StepType};
use crate::signal::domain::{SignalKind, SignalStatus};
use serde_json::json;

fn step(n: u32) -> ThinkStep {
    serde_json::from_value(json!({
        "step_number": n,
        "record_id": format!("step-{n}"),
        "purpose": format!("step {n}"),
        "timestamp": "2026-06-09T00:00:00Z",
    }))
    .unwrap()
}

fn progress(id: u32, step: &str) -> Progress {
    Progress {
        id,
        step_id: step.into(),
        timestamp: "2026-06-09T00:00:00Z".into(),
        progress_type: ProgressType::Code,
        description: format!("progress {id}"),
        files_touched: vec![],
        tools_used: vec![],
        result: String::new(),
        think_step: None,
    }
}

fn run_cycle(progress: Vec<Progress>) -> (Goal, Vec<Step>) {
    let goal = Goal {
        description: "test goal task:test-chunk".into(),
        acceptance_criteria: vec![],
        constraints: vec![],
        scope: String::new(),
        verification: VerificationMode::Full,
        status: GoalStatus::Open,
        project_id: "t".into(),
        created_at: Some("2026-06-09T00:00:00+00:00".into()),
        completed_at: None,
    };
    let step = Step {
        id: "t1".into(),
        title: "step one".into(),
        step_type: StepType::Implement,
        status: StepStatus::Active,
        estimate: None,
        started_at: Some("2026-06-09T00:01:00Z".into()),
        completed_at: None,
        artifacts: vec![],
        checks: vec![],
        progress,
        waiting_reason: None,
        think_branch: None,
    };
    (goal, vec![step])
}

fn task(id: &str) -> Task {
    Task {
        tier: None,
        id: id.into(),
        title: id.into(),
        name: crate::plan::name::derive(id),
        status: TaskStatus::Idea,
        priority: 100,
        description: String::new(),
        content: None,
        feature: None,
        notes: String::new(),
        acceptance: vec![],
        deps: vec![],
        refs: vec![],
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        retired_reason: None,
        waiting_on: None,
        project_id: None,
        created_at: "2026-06-09T00:00:00Z".into(),
        updated_at: "2026-06-09T00:00:00Z".into(),
    }
}

fn signal(id: &str) -> Signal {
    Signal {
        id: id.into(),
        kind: SignalKind::Bug,
        from: "tester".into(),
        body: "body".into(),
        content: None,
        created: "2026-06-09T00:00:00Z".into(),
        status: SignalStatus::New,
        enrichment: vec![],
        refs: vec![],
        surfaced_at: None,
        snooze_until: None,
        project_id: None,
    }
}

#[test]
fn collects_one_envelope_per_record_in_family_order() {
    let steps = [step(1), step(2)];
    let (goal, run_steps) = run_cycle(vec![progress(1, "t1")]);
    let tasks = [task("c1"), task("c2"), task("c3")];
    let signals = [signal("s1")];

    let envelopes = collect_envelopes(
        "tenant-x",
        steps.iter(),
        Some((&goal, run_steps.as_slice())),
        &plan_of(tasks.to_vec()),
        &signals,
    )
    .unwrap();

    // One envelope per record: 2 think + (1 goal + 1 step + 1 progress) + 3 + 1.
    assert_eq!(envelopes.len(), 9);
    // Stable family order: think, run, plan, signal.
    let families: Vec<Family> = envelopes.iter().map(|e| e.family).collect();
    assert_eq!(
        families,
        vec![
            Family::Think,
            Family::Think,
            Family::Run,
            Family::Run,
            Family::Run,
            Family::Plan,
            Family::Plan,
            Family::Plan,
            Family::Signal,
        ]
    );
    // Kinds + cycle-scoped run identities carried through.
    assert_eq!(envelopes[0].kind, Kind::ThinkStep);
    assert_eq!(envelopes[2].kind, Kind::Goal);
    assert_eq!(envelopes[2].id, "obj-2026-06-09T00:00:00-00:00");
    assert_eq!(envelopes[3].kind, Kind::RunStep);
    assert_eq!(envelopes[3].id, "obj-2026-06-09T00:00:00-00:00.t1");
    assert_eq!(envelopes[4].kind, Kind::Progress);
    assert_eq!(envelopes[4].id, "obj-2026-06-09T00:00:00-00:00.progress-1");
    assert_eq!(envelopes[5].kind, Kind::Task);
    assert_eq!(envelopes[8].kind, Kind::Signal);
    assert!(envelopes.iter().all(|e| e.tenant_id == "tenant-x"));
}

#[test]
fn counts_match_the_collected_envelopes() {
    let steps = [step(1)];
    let tasks = [task("c1"), task("c2")];
    let signals = [signal("s1"), signal("s2"), signal("s3")];

    let envelopes =
        collect_envelopes("t", steps.iter(), None, &plan_of(tasks.to_vec()), &signals).unwrap();
    let counts = BackfillCounts::from_envelopes(&envelopes);

    assert_eq!(
        counts,
        BackfillCounts {
            think: 1,
            run: 0,
            plan: 2,
            signal: 3,
        }
    );
    assert_eq!(counts.total(), 6);
    assert_eq!(counts.total(), envelopes.len());
}

#[test]
fn an_empty_identity_refuses_the_collection() {
    let tasks = [task("c1"), task(""), task("c2")];
    let error = collect_envelopes("t", std::iter::empty(), None, &plan_of(tasks.to_vec()), &[])
        .unwrap_err();
    assert_eq!(
        error.kind,
        magistr_workflow_contracts::ReadErrorKind::Identity
    );
}

#[test]
fn empty_corpus_collects_nothing() {
    let envelopes =
        collect_envelopes("t", std::iter::empty(), None, &plan_of(vec![]), &[]).unwrap();
    assert!(envelopes.is_empty());
    assert_eq!(BackfillCounts::from_envelopes(&envelopes).total(), 0);
}
