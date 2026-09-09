use magistr_domain::identity::{ProjectId, RunId};

use crate::{ApplicationError, CommandContext};

/// Validates routing identity only; construction is not an authority lease or
/// permission grant. The host admits an owner before supplying concrete ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectScope {
    project: ProjectId,
}

impl ProjectScope {
    pub fn new(project: ProjectId) -> Self {
        Self { project }
    }

    pub fn project(&self) -> &ProjectId {
        &self.project
    }

    pub fn ensure_run(&self, run: &RunId) -> Result<(), ApplicationError> {
        if run.project() != &self.project {
            return Err(ApplicationError::ProjectMismatch {
                expected: self.project.clone(),
                actual: run.project().clone(),
            });
        }
        Ok(())
    }

    pub fn ensure_command(&self, context: &CommandContext) -> Result<(), ApplicationError> {
        self.ensure_run(context.run())
    }
}

#[cfg(test)]
#[path = "project_scope.tests.rs"]
mod tests;
