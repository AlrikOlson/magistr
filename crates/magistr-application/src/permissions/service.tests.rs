use super::*;

use alloc::{boxed::Box, vec::Vec};
use core::num::NonZeroU32;
use magistr_domain::identity::{
    AttemptId, CommandId, DecisionId, ProjectId, RunId, SessionId, TurnId,
};

use crate::permissions::PermissionAnswer;
use crate::test_support::ready;
use crate::{Capability, CommandReceipt, PortFuture, Revision};

#[derive(Default)]
struct FakePermissionPort {
    calls: Vec<(CommandContext, PermissionCommand)>,
    answer: Option<Result<PermissionResponse, ApplicationError>>,
}

impl PermissionPort for FakePermissionPort {
    fn resolve<'a>(
        &'a mut self,
        context: &'a CommandContext,
        command: &'a PermissionCommand,
    ) -> PortFuture<'a, PermissionResponse> {
        self.calls.push((context.clone(), command.clone()));
        let answer = self
            .answer
            .take()
            .unwrap_or_else(|| Ok(response(context, command)));
        Box::pin(async move { answer })
    }
}

fn context(project: &str, run: &str) -> CommandContext {
    let project = ProjectId::new(project).unwrap();
    let run = RunId::new(project, run).unwrap();
    CommandContext {
        attempt: AttemptId::new(CommandId::new(run, "answer").unwrap(), NonZeroU32::MIN),
        expected_revision: Revision(4),
    }
}

fn command(context: &CommandContext) -> PermissionCommand {
    let session = SessionId::new(context.run().clone(), "session").unwrap();
    let turn = TurnId::new(session, "turn").unwrap();
    PermissionCommand {
        decision: DecisionId::new(turn, "decision").unwrap(),
        answer: PermissionAnswer::Select {
            option_id: "opaque offered choice".into(),
        },
    }
}

fn response(context: &CommandContext, command: &PermissionCommand) -> PermissionResponse {
    PermissionResponse {
        receipt: CommandReceipt {
            attempt: context.attempt.clone(),
            revision: Revision(5),
        },
        decision: command.decision.clone(),
        outcome: PermissionOutcome::Resolved {
            answer: command.answer.clone(),
        },
    }
}

#[test]
fn offered_choice_is_preserved_without_inventing_allow_semantics() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let command = command(&context);
    let mut port = FakePermissionPort::default();
    let result =
        ready(Permissions::new(&scope, &mut port).resolve(&context, command.clone())).unwrap();
    assert_eq!(result.decision, command.decision);
    assert_eq!(
        result.outcome,
        PermissionOutcome::Resolved {
            answer: command.answer.clone()
        }
    );
    assert_eq!(port.calls, [(context, command)]);
}

#[test]
fn foreign_decision_scope_is_refused_before_answering() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    for foreign in [
        self::context("other", "run"),
        self::context("project", "other"),
    ] {
        let mut port = FakePermissionPort::default();
        let result =
            ready(Permissions::new(&scope, &mut port).resolve(&context, command(&foreign)));
        assert!(matches!(
            result,
            Err(ApplicationError::ScopeMismatch { .. })
                | Err(ApplicationError::ProjectMismatch { .. })
        ));
        assert!(port.calls.is_empty());
    }
}

#[test]
fn a_response_cannot_answer_another_decision_or_substitute_an_option() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let command = command(&context);
    let mut wrong_decision = response(&context, &command);
    wrong_decision.decision = DecisionId::new(command.decision.turn().clone(), "another").unwrap();
    let mut wrong_answer = response(&context, &command);
    wrong_answer.outcome = PermissionOutcome::Resolved {
        answer: PermissionAnswer::Select {
            option_id: "unrequested-choice".into(),
        },
    };
    for answer in [wrong_decision, wrong_answer] {
        let mut port = FakePermissionPort {
            answer: Some(Ok(answer)),
            ..FakePermissionPort::default()
        };
        assert!(matches!(
            ready(Permissions::new(&scope, &mut port).resolve(&context, command.clone())),
            Err(ApplicationError::InvalidResponse { .. })
        ));
    }
}

#[test]
fn policy_denial_is_returned_without_submitting_another_answer() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let error = ApplicationError::CapabilityDenied {
        capability: Capability::ResolvePermission,
        reason: "protected path".into(),
    };
    let mut port = FakePermissionPort {
        answer: Some(Err(error.clone())),
        ..FakePermissionPort::default()
    };
    assert_eq!(
        ready(Permissions::new(&scope, &mut port).resolve(&context, command(&context))),
        Err(error)
    );
    assert_eq!(port.calls.len(), 1);
}

#[test]
fn cancellation_and_expiration_remain_distinct_from_selecting_an_option() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let mut command = command(&context);
    command.answer = PermissionAnswer::Cancel;
    let mut port = FakePermissionPort::default();
    assert_eq!(
        ready(Permissions::new(&scope, &mut port).resolve(&context, command.clone()))
            .unwrap()
            .outcome,
        PermissionOutcome::Resolved {
            answer: PermissionAnswer::Cancel
        }
    );

    let mut expired = response(&context, &command);
    expired.outcome = PermissionOutcome::Expired;
    let mut port = FakePermissionPort {
        answer: Some(Ok(expired)),
        ..FakePermissionPort::default()
    };
    assert_eq!(
        ready(Permissions::new(&scope, &mut port).resolve(&context, command))
            .unwrap()
            .outcome,
        PermissionOutcome::Expired
    );
}
