use tokio_util::sync::CancellationToken;

use super::{RunExecution, RunFuture, RunLimits};
use crate::contracts::WorkspaceError;

/// The execution seam: one admitted argv, no shell, hard limits, a cancel
/// token. The production implementation is [`super::ProcessExecutor`];
/// tests inject scripted answers so the decision surface stays pure. An
/// `Err` means the process never started (a missing program, a refused
/// spawn); everything after a successful spawn is data in [`RunExecution`].
pub trait RunExecutor: Send + Sync {
    fn execute(
        &self,
        argv: Vec<String>,
        limits: RunLimits,
        cancel: CancellationToken,
    ) -> RunFuture<'_, Result<RunExecution, WorkspaceError>>;
}
