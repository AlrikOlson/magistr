use alloc::string::String;
use core::num::NonZeroU32;

use crate::CommandContext;

/// Patch project-owned overrides; `None` preserves the existing setting.
/// Unrelated fields, policy/export controls and all other layers are preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationUpdate {
    pub command: CommandContext,
    pub project_name: Option<String>,
    pub max_iterations: Option<NonZeroU32>,
}
