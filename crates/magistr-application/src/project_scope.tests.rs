use super::*;
use crate::{CommandReceipt, Revision};
use core::num::NonZeroU32;
use magistr_domain::identity::{AttemptId, CommandId};

fn context(project: &str, run: &str, command: &str) -> CommandContext {
    let project = ProjectId::new(project).expect("project");
    let run = RunId::new(project, run).expect("run");
    let command = CommandId::new(run, command).expect("command");
    CommandContext {
        attempt: AttemptId::new(command, NonZeroU32::MIN),
        expected_revision: Revision(7),
    }
}

#[test]
fn same_local_keys_in_different_projects_cannot_route_a_command() {
    let request = context("one", "run", "command");
    let scope = ProjectScope::new(ProjectId::new("two").expect("project"));
    assert!(matches!(
        scope.ensure_command(&request),
        Err(ApplicationError::ProjectMismatch { .. })
    ));
}

#[test]
fn another_run_in_the_same_project_cannot_be_targeted() {
    let request = context("one", "run", "command");
    let other = context("one", "other-run", "command");
    assert!(matches!(
        request.ensure_run(other.run()),
        Err(ApplicationError::ScopeMismatch { .. })
    ));
}

#[test]
fn receipt_for_another_attempt_is_not_success() {
    let request = context("one", "run", "command");
    let mut attempt = request.attempt.clone();
    attempt = AttemptId::new(
        attempt.command().clone(),
        NonZeroU32::new(2).expect("attempt"),
    );
    let receipt = CommandReceipt {
        attempt,
        revision: Revision(8),
    };
    assert!(matches!(
        request.validate_receipt(&receipt),
        Err(ApplicationError::InvalidResponse { .. })
    ));
}

#[test]
fn stale_receipt_is_not_success() {
    let request = context("one", "run", "command");
    let receipt = CommandReceipt {
        attempt: request.attempt.clone(),
        revision: Revision(6),
    };
    assert!(matches!(
        request.validate_receipt(&receipt),
        Err(ApplicationError::InvalidResponse { .. })
    ));
}

#[test]
fn matching_scope_and_receipt_are_accepted() {
    let request = context("one", "run", "command");
    let scope = ProjectScope::new(request.run().project().clone());
    assert_eq!(scope.ensure_command(&request), Ok(()));
    assert_eq!(request.ensure_run(request.run()), Ok(()));
    assert_eq!(
        request.validate_receipt(&CommandReceipt {
            attempt: request.attempt.clone(),
            revision: Revision(7)
        }),
        Ok(())
    );
}
