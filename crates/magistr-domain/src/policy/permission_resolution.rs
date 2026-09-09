use alloc::string::String;

use super::{PermissionOutcome, PermissionSource, RiskTier};

/// One complete policy result, usable by every permission responder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionResolution {
    pub outcome: PermissionOutcome,
    pub risk: RiskTier,
    pub reason: String,
    pub source: PermissionSource,
    pub scope: String,
    pub policy_version: u64,
}
