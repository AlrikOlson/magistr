use crate::{ApplicationError, CommandReceipt, ProjectScope};

use super::{ChunkReference, CompleteChunk, StartChunk, WorkflowPort};

/// Project-bound access to curated native workflow operations.
pub struct Workflow<'a> {
    scope: &'a ProjectScope,
    port: &'a mut dyn WorkflowPort,
}

impl<'a> Workflow<'a> {
    pub fn new(scope: &'a ProjectScope, port: &'a mut dyn WorkflowPort) -> Self {
        Self { scope, port }
    }

    pub async fn start_chunk(
        &mut self,
        request: StartChunk,
    ) -> Result<CommandReceipt, ApplicationError> {
        self.scope.ensure_command(&request.command)?;
        self.ensure_chunk(&request.chunk)?;
        let context = request.command.clone();
        let receipt = self.port.start_chunk(request).await?;
        context.validate_receipt(&receipt)?;
        Ok(receipt)
    }

    pub async fn complete_chunk(
        &mut self,
        request: CompleteChunk,
    ) -> Result<CommandReceipt, ApplicationError> {
        self.scope.ensure_command(&request.command)?;
        self.ensure_chunk(&request.chunk)?;
        request
            .command
            .ensure_run(request.verification_task.run())?;
        let context = request.command.clone();
        let receipt = self.port.complete_chunk(request).await?;
        context.validate_receipt(&receipt)?;
        Ok(receipt)
    }

    fn ensure_chunk(&self, chunk: &ChunkReference) -> Result<(), ApplicationError> {
        if self.scope.project() != chunk.project() {
            return Err(ApplicationError::ProjectMismatch {
                expected: self.scope.project().clone(),
                actual: chunk.project().clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
