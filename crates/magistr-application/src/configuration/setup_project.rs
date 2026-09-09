use alloc::string::String;
use core::num::NonZeroU32;

use crate::CommandContext;

/// Initialize project-owned configuration only if it does not already exist.
/// The adapter verifies absence atomically; this is never an overwrite operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupProject {
    pub command: CommandContext,
    pub project_name: String,
    pub max_iterations: NonZeroU32,
}
