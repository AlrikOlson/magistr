use crate::{CommandReceipt, PortFuture};

use super::{CompleteChunk, StartChunk};

/// The narrow native lifecycle authority, supplied only after host admission.
///
/// Implementations must enforce capability and expected revision atomically,
/// resolve project-scoped chunk keys and run-scoped task associations, and
/// retain native prerequisite/lifecycle checks. Completion requires the named
/// task to exist, belong to the chunk, be complete, and carry applicable passing
/// standard and required custom gates plus acceptance-specific evidence; a
/// caller-provided reference is never itself verification evidence.
/// No method exports roadmap files. Ambiguous effects return `UnknownOutcome`
/// for reconciliation of the same attempt instead of an automatic retry.
pub trait WorkflowPort: Send {
    fn start_chunk(&mut self, request: StartChunk) -> PortFuture<'_, CommandReceipt>;

    fn complete_chunk(&mut self, request: CompleteChunk) -> PortFuture<'_, CommandReceipt>;
}
