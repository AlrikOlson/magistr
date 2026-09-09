use alloc::string::String;

use super::{PermissionMode, PolicyEffect};

/// Normalized observations supplied by a host to the deterministic policy service.
/// Classification, path resolution and configuration loading belong to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionContext {
    pub mode: PermissionMode,
    pub profile_effect: PolicyEffect,
    pub profile_reason: String,
    pub local_allowance: bool,
    pub workflow_allowance: bool,
    /// A protected-target refusal is an immutable floor.
    pub protected: bool,
    /// Exact request identity including project, model and operation context.
    pub scope: String,
    /// Host-supplied identity of every behavior-affecting policy input.
    pub policy_version: u64,
}
