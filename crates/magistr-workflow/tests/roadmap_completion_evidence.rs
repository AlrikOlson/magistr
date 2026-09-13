//! Completion governed by the ship run's receipts, on every roadmap route.

use std::sync::{Arc, Mutex};

use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::roadmap::application::{ChunkDraft, RoadmapError, RoadmapUseCases};
use magistr_workflow::roadmap::domain::ChunkStatus;
use magistr_workflow::ship::application::{
    CheckExecution, CheckOutcome, CheckRequest, EvidencePort, GateContract, ShipUseCases,
    SourceFreshness, SourceObservation, SourceWitness,
};
use magistr_workflow::ship::domain::{check::CheckType, task::TaskType};
use magistr_workflow::ship::engine::ShipEngine;

/// A source that never moves while a gate runs: the revision it was
/// observed at is the revision it validates.
struct Fixed(String);

impl SourceWitness for Fixed {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            workspace_root: "/w".into(),
            workspace_generation: None,
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: self.0.clone(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        Ok(SourceFreshness::Unchanged)
    }
}

/// A fake evidence capability whose source revision the test moves between
/// a gate and the completion that consults it.
struct Port {
    revision: Arc<Mutex<String>>,
}

impl EvidencePort for Port {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        Ok(Box::new(Fixed(self.revision.lock().unwrap().clone())))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(vec![GateContract {
            name: "test".into(),
            command: "cargo test".into(),
            check_type: CheckType::Test,
        }])
    }
}

fn chunk(id: &str, status: ChunkStatus) -> ChunkDraft {
    ChunkDraft {
        id: id.into(),
        title: format!("Chunk {id}"),
        name: String::new(),
        status,
        priority: 100,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        shared: false,
        group: None,
        content: None,
    }
}

/// A roadmap over `ship`, with `c1` started.
fn roadmap_over(ship: &ShipUseCases) -> RoadmapUseCases {
    let roadmap = RoadmapUseCases::new(
        Arc::new(Mutex::new(RoadmapEngine::new("project".into()))),
        ship.clone(),
    );
    roadmap
        .add_chunk(chunk("c1", ChunkStatus::Pending))
        .unwrap();
    roadmap.start_chunk("c1").unwrap();
    roadmap
}

/// The ship family with the fake evidence capability, and the handle that
/// moves its source revision.
fn evidenced() -> (ShipUseCases, Arc<Mutex<String>>) {
    let revision = Arc::new(Mutex::new("r1".to_owned()));
    let ship = ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("project".into()))))
        .with_evidence(Arc::new(Port {
            revision: Arc::clone(&revision),
        }));
    (ship, revision)
}

fn composed() -> (RoadmapUseCases, ShipUseCases) {
    let (ship, _) = evidenced();
    (roadmap_over(&ship), ship)
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
fn stale_source_evidence_is_refused_until_the_gate_reruns() {
    let (ship, revision) = evidenced();
    let roadmap = roadmap_over(&ship);
    objective(&ship, "chunk:c1");
    gate(&ship, 0);
    *revision.lock().unwrap() = "r2".into();
    let refusal = roadmap.complete_chunk("c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("the source changed since the gate ran"),
        "{refusal}"
    );
    assert_eq!(
        roadmap.engine().lock().unwrap().roadmap().chunks[0].status,
        ChunkStatus::InProgress
    );
    gate(&ship, 0);
    assert_eq!(
        roadmap.complete_chunk("c1", None).unwrap().status,
        ChunkStatus::Done
    );
}

/// A ship family composed without an evidence capability is still the
/// roadmap's completion authority — one that refuses every completion with
/// the reason. There is no permissive branch to fall back to: the
/// constructor takes the authority, and the plain status routes refuse
/// `done` whatever the ship family can certify.
#[test]
fn without_an_evidence_capability_every_route_to_done_is_refused_and_planning_continues() {
    let ship = ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("project".into()))));
    let roadmap = roadmap_over(&ship);
    objective(&ship, "chunk:c1");

    let refusal = roadmap
        .complete_chunk("c1", Some("task:verify"))
        .unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("no verification evidence capability is composed"),
        "{refusal}"
    );
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
    let chunks = roadmap.engine().lock().unwrap().roadmap().chunks.clone();
    assert_eq!(chunks[0].status, ChunkStatus::InProgress);
    assert!(
        chunks[0].cross_refs.is_empty(),
        "{:?}",
        chunks[0].cross_refs
    );

    // Ordinary planning mutations and completed history are untouched.
    roadmap.resolve_status_proposal("c1", false).unwrap();
    assert_eq!(
        roadmap
            .set_status("c1", ChunkStatus::Blocked)
            .unwrap()
            .status,
        ChunkStatus::Blocked
    );
    assert_eq!(
        roadmap.link_chunk("c1", "think:7").unwrap().cross_refs,
        vec!["think:7".to_owned()]
    );
    assert_eq!(
        roadmap
            .add_chunk(chunk("shipped-last-year", ChunkStatus::Done))
            .unwrap()
            .status,
        ChunkStatus::Done
    );
    assert_eq!(
        roadmap
            .obsolete_chunk("c1", "superseded".into())
            .unwrap()
            .status,
        ChunkStatus::Obsoleted
    );
}
