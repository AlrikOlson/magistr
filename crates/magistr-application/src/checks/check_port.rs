use crate::PortFuture;

use super::{CheckExecution, RunCheck};

/// Runs the project-configured command, never a caller-supplied shell string.
///
/// The host enforces `RunChecks`, the expected revision, task existence and run
/// association before execution, resolving the exact configured gate name and
/// checking its category. It records the actual gate result on that task
/// and returns a terminal outcome, distinguishing missing recording from failed
/// execution. Unknown execution outcome must retain the attempt for recovery;
/// implementers must not silently launch the gate again after cancellation.
pub trait CheckPort: Send {
    fn run_check(&mut self, request: RunCheck) -> PortFuture<'_, CheckExecution>;
}
