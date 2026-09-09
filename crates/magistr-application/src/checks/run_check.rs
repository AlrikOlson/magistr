use super::CheckTarget;
use crate::{CommandContext, workflow::TaskReference};

/// Execute a configured gate in a real native verification task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCheck {
    pub command: CommandContext,
    pub gate: CheckTarget,
    pub task: TaskReference,
}
