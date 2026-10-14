use alloc::string::String;
use magistr_domain::identity::{ProviderSessionBinding, SessionId, TurnId};

use crate::{ApplicationError, CommandContext, ProjectScope};

use super::{SessionCommand, SessionOutcome, SessionPort, SessionResponse};

/// Session use cases for one project, independent of provider and runtime SDKs.
pub struct Sessions<'a> {
    scope: &'a ProjectScope,
    port: &'a mut dyn SessionPort,
}

impl<'a> Sessions<'a> {
    #[must_use]
    pub fn new(scope: &'a ProjectScope, port: &'a mut dyn SessionPort) -> Self {
        Self { scope, port }
    }

    /// Create a fresh provider conversation for an independently allocated session.
    pub async fn start(
        &mut self,
        context: &CommandContext,
        session: SessionId,
        provider: String,
        model: Option<String>,
    ) -> Result<SessionResponse, ApplicationError> {
        self.execute(
            context,
            SessionCommand::Start {
                session,
                provider,
                model,
            },
        )
        .await
    }

    /// Restore a bound provider conversation only when the adapter supports it.
    pub async fn resume(
        &mut self,
        context: &CommandContext,
        binding: ProviderSessionBinding,
    ) -> Result<SessionResponse, ApplicationError> {
        self.execute(context, SessionCommand::Resume { binding })
            .await
    }

    /// Submit exact prompt content; acceptance is not terminal success.
    pub async fn prompt(
        &mut self,
        context: &CommandContext,
        turn: TurnId,
        content: super::PromptInput,
    ) -> Result<SessionResponse, ApplicationError> {
        self.execute(context, SessionCommand::Prompt { turn, content })
            .await
    }

    /// Request cancellation of a specific turn while retaining the session.
    pub async fn cancel_turn(
        &mut self,
        context: &CommandContext,
        turn: TurnId,
    ) -> Result<SessionResponse, ApplicationError> {
        self.execute(context, SessionCommand::CancelTurn { turn })
            .await
    }

    /// Close the session, with success requiring observed resource release.
    pub async fn close(
        &mut self,
        context: &CommandContext,
        session: SessionId,
    ) -> Result<SessionResponse, ApplicationError> {
        self.execute(context, SessionCommand::Close { session })
            .await
    }

    async fn execute(
        &mut self,
        context: &CommandContext,
        command: SessionCommand,
    ) -> Result<SessionResponse, ApplicationError> {
        self.scope.ensure_command(context)?;
        context.ensure_run(command.session().run())?;
        let response = self.port.execute(context, &command).await?;
        context.validate_receipt(&response.receipt)?;
        let matches = match (&command, &response.outcome) {
            (
                SessionCommand::Start {
                    session, provider, ..
                },
                SessionOutcome::SessionReady { binding },
            ) => binding.session() == session && binding.provider_session().provider() == provider,
            (
                SessionCommand::Resume { binding: requested },
                SessionOutcome::SessionReady { binding },
            ) => binding == requested,
            (
                SessionCommand::Prompt {
                    turn: requested, ..
                },
                SessionOutcome::TurnAccepted { turn },
            )
            | (
                SessionCommand::CancelTurn { turn: requested },
                SessionOutcome::CancellationRequested { turn },
            ) => turn == requested,
            (
                SessionCommand::Close { session: requested },
                SessionOutcome::SessionClosed { session },
            ) => session == requested,
            _ => false,
        };
        if !matches {
            return Err(ApplicationError::InvalidResponse {
                reason: "session outcome does not match its command target and operation".into(),
            });
        }
        Ok(response)
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
