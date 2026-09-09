use super::*;
use crate::projections::{EventSequence, OwnerGeneration, ProjectionFreshness};
use crate::{
    Revision,
    test_support::{ready, run},
};
use alloc::boxed::Box;
use magistr_domain::workflow::WorkflowPhase;

struct FakeProjection {
    calls: usize,
    result: Result<RunProjection, ApplicationError>,
}

impl ProjectionPort for FakeProjection {
    fn read_run(&mut self, _: &RunId) -> crate::PortFuture<'_, RunProjection> {
        self.calls += 1;
        Box::pin(core::future::ready(self.result.clone()))
    }
}

fn snapshot() -> RunProjection {
    RunProjection {
        run: run("project", "run"),
        revision: Revision(3),
        owner_generation: Some(OwnerGeneration(7)),
        event_sequence: Some(EventSequence(42)),
        freshness: ProjectionFreshness::Unknown,
        phase: Some(WorkflowPhase::Researching),
    }
}

#[test]
fn unknown_freshness_and_independent_cursors_survive_query() {
    let snapshot = snapshot();
    let scope = ProjectScope::new(snapshot.run.project().clone());
    let mut port = FakeProjection {
        calls: 0,
        result: Ok(snapshot.clone()),
    };
    let output = ready(Projections::new(&scope, &mut port).run(&snapshot.run)).unwrap();
    assert_eq!(output, snapshot);
    assert_eq!(port.calls, 1);
}

#[test]
fn foreign_requests_are_rejected_before_reading() {
    let snapshot = snapshot();
    let scope = ProjectScope::new(snapshot.run.project().clone());
    let mut port = FakeProjection {
        calls: 0,
        result: Ok(snapshot),
    };
    assert!(matches!(
        ready(Projections::new(&scope, &mut port).run(&run("foreign", "run"))),
        Err(ApplicationError::ProjectMismatch { .. })
    ));
    assert_eq!(port.calls, 0);
}

#[test]
fn another_run_response_is_rejected_even_with_same_project() {
    let snapshot = snapshot();
    let scope = ProjectScope::new(snapshot.run.project().clone());
    let mut port = FakeProjection {
        calls: 0,
        result: Ok(snapshot),
    };
    assert!(matches!(
        ready(Projections::new(&scope, &mut port).run(&run("project", "other"))),
        Err(ApplicationError::InvalidResponse { .. })
    ));
}

#[test]
fn persistence_error_is_preserved() {
    let current = run("project", "run");
    let scope = ProjectScope::new(current.project().clone());
    let error = ApplicationError::PersistenceFailure {
        operation: "read run projection".into(),
    };
    let mut port = FakeProjection {
        calls: 0,
        result: Err(error.clone()),
    };
    assert_eq!(
        ready(Projections::new(&scope, &mut port).run(&current)),
        Err(error)
    );
    assert_eq!(port.calls, 1);
}

#[test]
fn event_position_without_its_owner_generation_is_rejected() {
    let mut snapshot = snapshot();
    let current = snapshot.run.clone();
    let scope = ProjectScope::new(current.project().clone());
    snapshot.owner_generation = None;
    let mut port = FakeProjection {
        calls: 0,
        result: Ok(snapshot),
    };
    assert!(matches!(
        ready(Projections::new(&scope, &mut port).run(&current)),
        Err(ApplicationError::InvalidResponse { .. })
    ));
}
