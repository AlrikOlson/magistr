use magistr_domain::identity::ProjectId;

use crate::{ApplicationError, CommandReceipt, ProjectScope};

use super::{ConfigurationPort, ConfigurationSnapshot, ConfigurationUpdate, SetupProject};

/// Project-bound setup/configuration access without exposing storage formats.
pub struct Configuration<'a> {
    scope: &'a ProjectScope,
    port: &'a mut dyn ConfigurationPort,
}

impl<'a> Configuration<'a> {
    pub fn new(scope: &'a ProjectScope, port: &'a mut dyn ConfigurationPort) -> Self {
        Self { scope, port }
    }

    pub async fn read(&mut self) -> Result<ConfigurationSnapshot, ApplicationError> {
        let snapshot = self.port.read(self.scope.project()).await?;
        self.ensure_project(&snapshot.project)?;
        if snapshot.settings.project_name.value.trim().is_empty() {
            return Err(ApplicationError::InvalidResponse {
                reason: "resolved project name must not be empty".into(),
            });
        }
        Ok(snapshot)
    }

    pub async fn update(
        &mut self,
        request: ConfigurationUpdate,
    ) -> Result<CommandReceipt, ApplicationError> {
        self.scope.ensure_command(&request.command)?;
        if request
            .project_name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
            || (request.project_name.is_none() && request.max_iterations.is_none())
        {
            return Err(ApplicationError::InvalidRequest {
                reason: "configuration patch needs a setting and a nonempty project name".into(),
            });
        }
        let context = request.command.clone();
        let receipt = self.port.update(request).await?;
        context.validate_receipt(&receipt)?;
        Ok(receipt)
    }

    pub async fn setup(
        &mut self,
        request: SetupProject,
    ) -> Result<CommandReceipt, ApplicationError> {
        self.scope.ensure_command(&request.command)?;
        if request.project_name.trim().is_empty() {
            return Err(ApplicationError::InvalidRequest {
                reason: "project setup requires a nonempty project name".into(),
            });
        }
        let context = request.command.clone();
        let receipt = self.port.setup(request).await?;
        context.validate_receipt(&receipt)?;
        Ok(receipt)
    }

    fn ensure_project(&self, project: &ProjectId) -> Result<(), ApplicationError> {
        if project != self.scope.project() {
            return Err(ApplicationError::InvalidResponse {
                reason: "configuration snapshot belongs to another project".into(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
