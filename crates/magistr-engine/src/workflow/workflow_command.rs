use serde::{Deserialize, Serialize};
use std::fmt;

/// Commands that can be sent to the workflow engine.
///
/// These control the iteration loop lifecycle. Commands are validated
/// against the current engine state before execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowCommand {
    /// Begin a new iteration (resets phase to Researching).
    StartIteration,
    /// Pause the loop after the current iteration completes.
    Pause,
    /// Resume a paused loop.
    Resume,
    /// Skip the current iteration (advance without commit).
    Skip,
    /// Abort the loop entirely.
    AbortLoop,
}

impl WorkflowCommand {
    /// Human-readable name for this command variant.
    pub fn name(&self) -> &'static str {
        match self {
            Self::StartIteration => "start_iteration",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Skip => "skip",
            Self::AbortLoop => "abort_loop",
        }
    }
}

impl fmt::Display for WorkflowCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
