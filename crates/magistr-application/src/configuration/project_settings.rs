use alloc::string::String;
use core::num::NonZeroU32;

use super::ResolvedValue;

/// Curated display and iteration defaults shared by setup and hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSettings {
    pub project_name: ResolvedValue<String>,
    pub max_iterations: ResolvedValue<NonZeroU32>,
}
