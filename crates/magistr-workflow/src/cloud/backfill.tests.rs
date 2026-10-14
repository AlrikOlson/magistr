use super::*;
use crate::cloud::envelope::Kind;
use crate::roadmap::domain::{Chunk, ChunkStatus};

/// A roadmap with the supplied chunks and no tracker state.
fn roadmap_of(chunks: Vec<Chunk>) -> Roadmap {
    Roadmap {
        project_id: "t".into(),
        chunks,
        ..Roadmap::default()
    }
}
use crate::ship::domain::action::{Action, ActionType};
use crate::ship::domain::objective::{ObjectiveStatus, VerificationMode};
use crate::ship::domain::task::{TaskStatus, TaskType};
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

fn action(id: u32, task: &str) -> Action {
    Action {
        id,
        task_id: task.into(),
        timestamp: "2026-06-09T00:00:00Z".into(),
        action_type: ActionType::Code,
        description: format!("action {id}"),
        files_touched: vec![],
        tools_used: vec![],
        result: String::new(),
        think_step: None,
    }
}

fn ship_cycle(actions: Vec<Action>) -> (Objective, Vec<Task>) {
    let objective = Objective {
        description: "test objective chunk:test-chunk".into(),
        acceptance_criteria: vec![],
        constraints: vec![],
        scope: String::new(),
        verification: VerificationMode::Full,
        status: ObjectiveStatus::Defined,
        project_id: "t".into(),
        created_at: Some("2026-06-09T00:00:00+00:00".into()),
        completed_at: None,
    };
    let task = Task {
        id: "t1".into(),
        title: "task one".into(),
        task_type: TaskType::Implement,
        status: TaskStatus::Active,
        estimate: None,
        started_at: Some("2026-06-09T00:01:00Z".into()),
        completed_at: None,
        artifacts: vec![],
        checks: vec![],
        actions,
        blocked_reason: None,
        think_branch: None,
    };
    (objective, vec![task])
}

fn chunk(id: &str) -> Chunk {
    Chunk {
        tier: None,
        id: id.into(),
        title: id.into(),
        name: crate::roadmap::name::derive(id),
        status: ChunkStatus::Backlog,
        priority: 100,
        description: String::new(),
        content: None,
        group: None,
        notes: String::new(),
        acceptance: vec![],
        deps: vec![],
        cross_refs: vec![],
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        obsoleted_reason: None,
        blocked_by: None,
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
        cross_refs: vec![],
        surfaced_at: None,
        snooze_until: None,
        project_id: None,
    }
}

#[test]
fn collects_one_envelope_per_record_in_family_order() {
    let steps = [step(1), step(2)];
    let (objective, tasks) = ship_cycle(vec![action(1, "t1")]);
    let chunks = [chunk("c1"), chunk("c2"), chunk("c3")];
    let signals = [signal("s1")];

    let envelopes = collect_envelopes(
        "tenant-x",
        steps.iter(),
        Some((&objective, tasks.as_slice())),
        &roadmap_of(chunks.to_vec()),
        &signals,
    )
    .unwrap();

    // One envelope per record: 2 think + (1 objective + 1 task + 1 action) + 3 + 1.
    assert_eq!(envelopes.len(), 9);
    // Stable family order: think, ship, roadmap, signal.
    let families: Vec<Family> = envelopes.iter().map(|e| e.family).collect();
    assert_eq!(
        families,
        vec![
            Family::Think,
            Family::Think,
            Family::Ship,
            Family::Ship,
            Family::Ship,
            Family::Roadmap,
            Family::Roadmap,
            Family::Roadmap,
            Family::Signal,
        ]
    );
    // Kinds + cycle-scoped ship identities carried through.
    assert_eq!(envelopes[0].kind, Kind::Step);
    assert_eq!(envelopes[2].kind, Kind::Objective);
    assert_eq!(envelopes[2].id, "obj-2026-06-09T00:00:00-00:00");
    assert_eq!(envelopes[3].kind, Kind::Task);
    assert_eq!(envelopes[3].id, "obj-2026-06-09T00:00:00-00:00.t1");
    assert_eq!(envelopes[4].kind, Kind::Action);
    assert_eq!(envelopes[4].id, "obj-2026-06-09T00:00:00-00:00.action-1");
    assert_eq!(envelopes[5].kind, Kind::Chunk);
    assert_eq!(envelopes[8].kind, Kind::Signal);
    assert!(envelopes.iter().all(|e| e.tenant_id == "tenant-x"));
}

#[test]
fn counts_match_the_collected_envelopes() {
    let steps = [step(1)];
    let chunks = [chunk("c1"), chunk("c2")];
    let signals = [signal("s1"), signal("s2"), signal("s3")];

    let envelopes = collect_envelopes(
        "t",
        steps.iter(),
        None,
        &roadmap_of(chunks.to_vec()),
        &signals,
    )
    .unwrap();
    let counts = BackfillCounts::from_envelopes(&envelopes);

    assert_eq!(
        counts,
        BackfillCounts {
            think: 1,
            ship: 0,
            roadmap: 2,
            signal: 3,
        }
    );
    assert_eq!(counts.total(), 6);
    assert_eq!(counts.total(), envelopes.len());
}

#[test]
fn an_empty_identity_refuses_the_collection() {
    let chunks = [chunk("c1"), chunk(""), chunk("c2")];
    let error = collect_envelopes(
        "t",
        std::iter::empty(),
        None,
        &roadmap_of(chunks.to_vec()),
        &[],
    )
    .unwrap_err();
    assert_eq!(
        error.kind,
        magistr_workflow_contracts::ReadErrorKind::Identity
    );
}

#[test]
fn empty_corpus_collects_nothing() {
    let envelopes =
        collect_envelopes("t", std::iter::empty(), None, &roadmap_of(vec![]), &[]).unwrap();
    assert!(envelopes.is_empty());
    assert_eq!(BackfillCounts::from_envelopes(&envelopes).total(), 0);
}
