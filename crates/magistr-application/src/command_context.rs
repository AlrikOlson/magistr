use magistr_domain::identity::{AttemptId, RunId};

use crate::{ApplicationError, CommandReceipt, Revision};

/// Adapter-allocated attempt and optimistic precondition. Ports must check the
/// revision atomically with the operation; a prior query is not admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContext {
    pub attempt: AttemptId,
    pub expected_revision: Revision,
}

impl CommandContext {
    pub fn run(&self) -> &RunId {
        self.attempt.command().run()
    }

    pub fn ensure_run(&self, run: &RunId) -> Result<(), ApplicationError> {
        if self.run() != run {
            return Err(ApplicationError::ScopeMismatch {
                expected: self.run().clone(),
                actual: run.clone(),
            });
        }
        Ok(())
    }

    pub fn validate_receipt(&self, receipt: &CommandReceipt) -> Result<(), ApplicationError> {
        if receipt.attempt != self.attempt {
            return Err(ApplicationError::InvalidResponse {
                reason: "receipt belongs to another command attempt".into(),
            });
        }
        if receipt.revision < self.expected_revision {
            return Err(ApplicationError::InvalidResponse {
                reason: "receipt predates the expected revision".into(),
            });
        }
        Ok(())
    }
}
