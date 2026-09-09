use super::*;
use crate::workflow::TaskReference;
use crate::{
    Revision,
    test_support::{command, ready, run},
};
use alloc::boxed::Box;

struct FakeWorkflow {
    calls: usize,
    result: Result<CommandReceipt, ApplicationError>,
}

impl WorkflowPort for FakeWorkflow {
    fn start_chunk(&mut self, _: StartChunk) -> crate::PortFuture<'_, CommandReceipt> {
        self.calls += 1;
        Box::pin(core::future::ready(self.result.clone()))
    }

    fn complete_chunk(&mut self, _: CompleteChunk) -> crate::PortFuture<'_, CommandReceipt> {
        self.calls += 1;
        Box::pin(core::future::ready(self.result.clone()))
    }
}

#[test]
fn completion_rejects_a_task_from_another_run_before_effects() {
    let current = run("project", "current");
    let context = command(&current);
    let scope = ProjectScope::new(current.project().clone());
    let mut port = FakeWorkflow {
        calls: 0,
        result: Ok(CommandReceipt {
            attempt: context.attempt.clone(),
            revision: Revision(8),
        }),
    };
    let request = CompleteChunk {
        command: context,
        chunk: ChunkReference::new(current.project().clone(), "arch-04").unwrap(),
        verification_task: TaskReference::new(run("project", "other"), "verification").unwrap(),
    };
    let error = ready(Workflow::new(&scope, &mut port).complete_chunk(request)).unwrap_err();
    assert!(matches!(error, ApplicationError::ScopeMismatch { .. }));
    assert_eq!(port.calls, 0);
}

#[test]
fn start_rejects_a_chunk_from_another_project_before_effects() {
    let current = run("project", "run");
    let context = command(&current);
    let scope = ProjectScope::new(current.project().clone());
    let mut port = FakeWorkflow {
        calls: 0,
        result: Ok(CommandReceipt {
            attempt: context.attempt.clone(),
            revision: Revision(8),
        }),
    };
    let request = StartChunk {
        command: context,
        chunk: ChunkReference::new(run("foreign", "run").project().clone(), "arch-04").unwrap(),
    };
    assert!(matches!(
        ready(Workflow::new(&scope, &mut port).start_chunk(request)),
        Err(ApplicationError::ProjectMismatch { .. })
    ));
    assert_eq!(port.calls, 0);
}

#[test]
fn completion_forwards_persistence_failure_without_retrying() {
    let current = run("project", "run");
    let scope = ProjectScope::new(current.project().clone());
    let error = ApplicationError::PersistenceFailure {
        operation: "complete chunk".into(),
    };
    let mut port = FakeWorkflow {
        calls: 0,
        result: Err(error.clone()),
    };
    let request = CompleteChunk {
        command: command(&current),
        chunk: ChunkReference::new(current.project().clone(), "arch-04").unwrap(),
        verification_task: TaskReference::new(current, "verification").unwrap(),
    };
    assert_eq!(
        ready(Workflow::new(&scope, &mut port).complete_chunk(request)),
        Err(error)
    );
    assert_eq!(port.calls, 1);
}

#[test]
fn start_rejects_a_receipt_for_another_attempt() {
    let current = run("project", "run");
    let scope = ProjectScope::new(current.project().clone());
    let mut port = FakeWorkflow {
        calls: 0,
        result: Ok(CommandReceipt {
            attempt: command(&run("project", "other")).attempt,
            revision: Revision(8),
        }),
    };
    let request = StartChunk {
        command: command(&current),
        chunk: ChunkReference::new(current.project().clone(), "arch-04").unwrap(),
    };
    assert!(ready(Workflow::new(&scope, &mut port).start_chunk(request)).is_err());
    assert_eq!(port.calls, 1);
}

#[test]
fn references_reject_empty_and_ambiguous_keys() {
    let current = run("project", "run");
    assert!(ChunkReference::new(current.project().clone(), " ").is_err());
    assert!(TaskReference::new(current, "task:verification").is_err());
}

#[test]
fn completion_accepts_a_correlated_receipt() {
    let current = run("project", "run");
    let context = command(&current);
    let scope = ProjectScope::new(current.project().clone());
    let receipt = CommandReceipt {
        attempt: context.attempt.clone(),
        revision: Revision(8),
    };
    let mut port = FakeWorkflow {
        calls: 0,
        result: Ok(receipt.clone()),
    };
    let request = CompleteChunk {
        command: context,
        chunk: ChunkReference::new(current.project().clone(), "arch-04").unwrap(),
        verification_task: TaskReference::new(current, "verification").unwrap(),
    };
    assert_eq!(
        ready(Workflow::new(&scope, &mut port).complete_chunk(request)),
        Ok(receipt)
    );
    assert_eq!(port.calls, 1);
}
