//! Completion governed by the ship run's receipts, on every roadmap route.

use std::sync::{Arc, Mutex};

use think_and_ship::roadmap::RoadmapEngine;
use think_and_ship::roadmap::application::{ChunkDraft, RoadmapError, RoadmapUseCases};
use think_and_ship::roadmap::domain::ChunkStatus;
use think_and_ship::ship::application::{
    CheckExecution, CheckOutcome, CheckRequest, EvidencePort, GateContract, ShipUseCases,
    SourceFreshness, SourceObservation, SourceWitness,
};
use think_and_ship::ship::domain::{check::CheckType, task::TaskType};
use think_and_ship::ship::engine::ShipEngine;

struct Fixed;

impl SourceWitness for Fixed {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            workspace_root: "/w".into(),
            workspace_generation: None,
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: "r1".into(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        Ok(SourceFreshness::Unchanged)
    }
}

struct Port;

impl EvidencePort for Port {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        Ok(Box::new(Fixed))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(vec![GateContract {
            name: "test".into(),
            command: "cargo test".into(),
            check_type: CheckType::Test,
        }])
    }
}

fn composed() -> (RoadmapUseCases, ShipUseCases) {
    let ship = ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("project".into()))))
        .with_evidence(Arc::new(Port));
    let roadmap = RoadmapUseCases::new(Arc::new(Mutex::new(RoadmapEngine::new("project".into()))))
        .with_completion_evidence(ship.clone());
    roadmap
        .add_chunk(ChunkDraft {
            id: "c1".into(),
            title: "Chunk one".into(),
            name: String::new(),
            status: ChunkStatus::Pending,
            priority: 100,
            description: String::new(),
            acceptance: vec![],
            deps: vec![],
            shared: false,
            group: None,
            content: None,
        })
        .unwrap();
    roadmap.start_chunk("c1").unwrap();
    (roadmap, ship)
}

fn objective(ship: &ShipUseCases, scope: &str) {
    ship.set_objective("Do c1".into(), vec![], vec![], scope.into())
        .unwrap();
    ship.add_task(
        "verify".into(),
        "Verify".into(),
        TaskType::Review,
        None,
        None,
    )
    .unwrap();
    ship.start_task("verify").unwrap();
}

fn gate(ship: &ShipUseCases, code: i32) {
    let admission = ship
        .admit_check(CheckRequest {
            task_id: None,
            check_type: CheckType::Test,
            name: "test".into(),
            passed: None,
            details: String::new(),
            required: true,
            command: Some("cargo test".into()),
            report: None,
        })
        .unwrap();
    ship.settle_check(
        admission,
        CheckOutcome {
            passed: code == 0,
            details: String::new(),
            verified: true,
            command: Some("cargo test".into()),
            exit_code: Some(code),
            execution: CheckExecution::Exited { code },
            report: None,
            results: None,
        },
    )
    .unwrap();
}

#[test]
fn completion_refuses_without_receipts_and_names_the_resolved_task_when_it_completes() {
    let (roadmap, ship) = composed();
    let refusal = roadmap.complete_chunk("c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("no ship objective"),
        "{refusal}"
    );

    objective(&ship, "chunk:c1");
    let refusal = roadmap.complete_chunk("c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("no check was recorded"),
        "{refusal}"
    );

    gate(&ship, 1);
    let refusal = roadmap
        .complete_chunk("c1", Some("task:verify"))
        .unwrap_err();
    let text = refusal.to_string();
    assert!(
        text.contains("test: the latest attempt did not pass (exit 1)"),
        "{text}"
    );
    assert!(
        text.contains("Re-run the outstanding gates on `verify`"),
        "{text}"
    );
    assert_eq!(
        roadmap.engine().lock().unwrap().roadmap().chunks[0].status,
        ChunkStatus::InProgress
    );

    gate(&ship, 0);
    let done = roadmap.complete_chunk("c1", None).unwrap();
    assert_eq!(done.status, ChunkStatus::Done);
    assert!(
        done.cross_refs.iter().any(|r| r == "task:verify"),
        "{:?}",
        done.cross_refs
    );
}

#[test]
fn a_foreign_scope_or_task_is_refused_by_name() {
    let (roadmap, ship) = composed();
    objective(&ship, "chunk:other");
    gate(&ship, 0);
    let refusal = roadmap.complete_chunk("c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("scoped to `chunk:other`"),
        "{refusal}"
    );
    ship.set_objective("Do c1".into(), vec![], vec![], "chunk:c1".into())
        .unwrap();
    let refusal = roadmap
        .complete_chunk("c1", Some("task:ghost"))
        .unwrap_err();
    assert!(
        refusal.to_string().contains("task `ghost` is not a task"),
        "{refusal}"
    );
}

#[test]
fn the_plain_status_routes_cannot_reach_done() {
    let (roadmap, _ship) = composed();
    assert!(matches!(
        roadmap.set_status("c1", ChunkStatus::Done),
        Err(RoadmapError::InvalidState(reason)) if reason.contains("roadmap_complete_chunk")
    ));
    roadmap
        .propose_status(
            "c1",
            ChunkStatus::Done,
            "ticket closed".into(),
            "tracker".into(),
        )
        .unwrap();
    assert!(matches!(
        roadmap.resolve_status_proposal("c1", true),
        Err(RoadmapError::InvalidState(reason)) if reason.contains("roadmap_complete_chunk")
    ));
    // Rejecting the proposal, and every other status, stays open.
    roadmap.resolve_status_proposal("c1", false).unwrap();
    roadmap.set_status("c1", ChunkStatus::Blocked).unwrap();
}

#[test]
fn without_a_completion_authority_the_legacy_routes_stay_as_they_were() {
    let roadmap = RoadmapUseCases::new(Arc::new(Mutex::new(RoadmapEngine::new("project".into()))));
    roadmap
        .add_chunk(ChunkDraft {
            id: "c1".into(),
            title: "Chunk one".into(),
            name: String::new(),
            status: ChunkStatus::Pending,
            priority: 100,
            description: String::new(),
            acceptance: vec![],
            deps: vec![],
            shared: false,
            group: None,
            content: None,
        })
        .unwrap();
    roadmap.start_chunk("c1").unwrap();
    assert_eq!(
        roadmap.complete_chunk("c1", None).unwrap().status,
        ChunkStatus::Done
    );
}
