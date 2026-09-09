use super::*;

use alloc::{boxed::Box, vec::Vec};
use core::num::NonZeroU32;
use magistr_domain::identity::{AttemptId, CommandId, ProjectId, ProviderSessionId, RunId};

use crate::test_support::ready;
use crate::{CommandReceipt, PortFuture, Revision};

#[derive(Default)]
struct FakeSessionPort {
    calls: Vec<(CommandContext, SessionCommand)>,
    answer: Option<Result<SessionResponse, ApplicationError>>,
}

impl SessionPort for FakeSessionPort {
    fn execute<'a>(
        &'a mut self,
        context: &'a CommandContext,
        command: &'a SessionCommand,
    ) -> PortFuture<'a, SessionResponse> {
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
        attempt: AttemptId::new(CommandId::new(run, "command").unwrap(), NonZeroU32::MIN),
        expected_revision: Revision(4),
    }
}

fn session(context: &CommandContext) -> SessionId {
    SessionId::new(context.run().clone(), "session").unwrap()
}

fn response(context: &CommandContext, command: &SessionCommand) -> SessionResponse {
    let outcome = match command {
        SessionCommand::Start {
            session, provider, ..
        } => SessionOutcome::SessionReady {
            binding: ProviderSessionId::new(provider.clone(), "conversation")
                .unwrap()
                .bind_to(session.clone()),
        },
        SessionCommand::Resume { binding } => SessionOutcome::SessionReady {
            binding: binding.clone(),
        },
        SessionCommand::Prompt { turn, .. } => SessionOutcome::TurnAccepted { turn: turn.clone() },
        SessionCommand::CancelTurn { turn } => {
            SessionOutcome::CancellationRequested { turn: turn.clone() }
        }
        SessionCommand::Close { session } => SessionOutcome::SessionClosed {
            session: session.clone(),
        },
    };
    SessionResponse {
        receipt: CommandReceipt {
            attempt: context.attempt.clone(),
            revision: Revision(5),
        },
        outcome,
    }
}

#[test]
fn concrete_use_cases_preserve_identity_and_distinguish_acceptance_from_termination() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let session = session(&context);
    let turn = TurnId::new(session.clone(), "turn").unwrap();
    let mut port = FakeSessionPort::default();
    let mut sessions = Sessions::new(&scope, &mut port);
    let started = ready(sessions.start(
        &context,
        session.clone(),
        "provider".into(),
        Some("model".into()),
    ))
    .unwrap();
    let SessionOutcome::SessionReady { binding } = started.outcome else {
        panic!("session was not ready")
    };
    assert_eq!(binding.session(), &session);
    assert_eq!(
        ready(sessions.resume(&context, binding.clone()))
            .unwrap()
            .outcome,
        SessionOutcome::SessionReady { binding }
    );
    assert_eq!(
        ready(sessions.prompt(&context, turn.clone(), "text".into()))
            .unwrap()
            .outcome,
        SessionOutcome::TurnAccepted { turn: turn.clone() }
    );
    assert_eq!(
        ready(sessions.cancel_turn(&context, turn.clone()))
            .unwrap()
            .outcome,
        SessionOutcome::CancellationRequested { turn }
    );
    assert_eq!(
        ready(sessions.close(&context, session.clone()))
            .unwrap()
            .outcome,
        SessionOutcome::SessionClosed { session }
    );
    assert_eq!(port.calls.len(), 5);
    assert!(port.calls.iter().all(|(seen, _)| seen == &context));
    assert!(
        matches!(&port.calls[0].1, SessionCommand::Start { model: Some(model), .. } if model == "model")
    );
}

#[test]
fn foreign_project_or_run_is_refused_before_session_dispatch() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    for foreign in [
        self::context("other", "run"),
        self::context("project", "other"),
    ] {
        let mut port = FakeSessionPort::default();
        let result = ready(Sessions::new(&scope, &mut port).close(&context, session(&foreign)));
        assert!(matches!(
            result,
            Err(ApplicationError::ScopeMismatch { .. })
                | Err(ApplicationError::ProjectMismatch { .. })
        ));
        assert!(port.calls.is_empty());
    }
    let foreign_scope = ProjectScope::new(ProjectId::new("other").unwrap());
    let mut port = FakeSessionPort::default();
    assert!(matches!(
        ready(Sessions::new(&foreign_scope, &mut port).close(&context, session(&context))),
        Err(ApplicationError::ProjectMismatch { .. })
    ));
    assert!(port.calls.is_empty());
}

#[test]
fn a_receipt_for_another_attempt_or_session_is_refused() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let command = SessionCommand::Close {
        session: session(&context),
    };
    let mut wrong_attempt = response(&context, &command);
    wrong_attempt.receipt.attempt = self::context("project", "other").attempt;
    let mut wrong_session = response(&context, &command);
    wrong_session.outcome = SessionOutcome::SessionClosed {
        session: SessionId::new(context.run().clone(), "another").unwrap(),
    };
    for answer in [wrong_attempt, wrong_session] {
        let mut port = FakeSessionPort {
            answer: Some(Ok(answer)),
            ..FakeSessionPort::default()
        };
        assert!(matches!(
            ready(Sessions::new(&scope, &mut port).close(&context, session(&context))),
            Err(ApplicationError::InvalidResponse { .. })
        ));
    }
}

#[test]
fn unknown_provider_outcome_is_not_reported_as_disconnect_or_retried() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    for error in [
        ApplicationError::UnknownOutcome {
            attempt: context.attempt.clone(),
        },
        ApplicationError::ProviderDisconnected {
            session: session(&context),
        },
    ] {
        let mut port = FakeSessionPort {
            answer: Some(Err(error.clone())),
            ..FakeSessionPort::default()
        };
        assert_eq!(
            ready(Sessions::new(&scope, &mut port).close(&context, session(&context))),
            Err(error)
        );
        assert_eq!(port.calls.len(), 1);
    }
}

#[test]
fn creation_rejects_a_different_provider_or_a_terminal_response_to_the_wrong_operation() {
    let context = context("project", "run");
    let scope = ProjectScope::new(context.run().project().clone());
    let session = session(&context);
    let command = SessionCommand::Start {
        session: session.clone(),
        provider: "requested".into(),
        model: None,
    };
    let mut wrong_provider = response(&context, &command);
    wrong_provider.outcome = SessionOutcome::SessionReady {
        binding: ProviderSessionId::new("other", "conversation")
            .unwrap()
            .bind_to(session.clone()),
    };
    let mut wrong_operation = response(&context, &command);
    wrong_operation.outcome = SessionOutcome::SessionClosed {
        session: session.clone(),
    };
    for answer in [wrong_provider, wrong_operation] {
        let mut port = FakeSessionPort {
            answer: Some(Ok(answer)),
            ..FakeSessionPort::default()
        };
        assert!(matches!(
            ready(Sessions::new(&scope, &mut port).start(
                &context,
                session.clone(),
                "requested".into(),
                None
            )),
            Err(ApplicationError::InvalidResponse { .. })
        ));
    }
}
