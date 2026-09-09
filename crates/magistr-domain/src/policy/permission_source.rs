use alloc::string::String;

/// The policy layer that determined an authorization outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionSource {
    Profile,
    LocalAllowance,
    WorkflowAllowance,
    ProtectedPath,
    Charter(String),
    Governance(String),
}
