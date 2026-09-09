use crate::{ApplicationError, CommandContext, ProjectScope};

use super::{PermissionCommand, PermissionOutcome, PermissionPort, PermissionResponse};

/// Permission resolution scoped to one application's project.
pub struct Permissions<'a> {
    scope: &'a ProjectScope,
    port: &'a mut dyn PermissionPort,
}

impl<'a> Permissions<'a> {
    #[must_use]
    pub fn new(scope: &'a ProjectScope, port: &'a mut dyn PermissionPort) -> Self {
        Self { scope, port }
    }

    /// Resolve a pending decision while retaining its requesting turn's identity.
    pub async fn resolve(
        &mut self,
        context: &CommandContext,
        command: PermissionCommand,
    ) -> Result<PermissionResponse, ApplicationError> {
        self.scope.ensure_command(context)?;
        context.ensure_run(command.decision.turn().session().run())?;
        let response = self.port.resolve(context, &command).await?;
        context.validate_receipt(&response.receipt)?;
        if response.decision != command.decision
            || matches!(&response.outcome, PermissionOutcome::Resolved { answer } if answer != &command.answer)
        {
            return Err(ApplicationError::InvalidResponse {
                reason: "permission outcome does not match its decision and requested answer"
                    .into(),
            });
        }
        Ok(response)
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
