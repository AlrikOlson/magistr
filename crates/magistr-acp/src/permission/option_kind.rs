//! What a permission option means.

use agent_client_protocol::schema::v1::PermissionOptionKind;

/// Permission option kinds, mirroring the protocol's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPermissionOptionKind {
    /// Allow this single tool use.
    AllowOnce,
    /// Allow this tool for the rest of the session.
    AllowAlways,
    /// Reject this single tool use.
    RejectOnce,
    /// Reject this tool for the rest of the session.
    RejectAlways,
}

impl From<PermissionOptionKind> for AcpPermissionOptionKind {
    fn from(kind: PermissionOptionKind) -> Self {
        match kind {
            PermissionOptionKind::AllowOnce => Self::AllowOnce,
            PermissionOptionKind::AllowAlways => Self::AllowAlways,
            PermissionOptionKind::RejectOnce => Self::RejectOnce,
            PermissionOptionKind::RejectAlways => Self::RejectAlways,
            // Non-exhaustive upstream — an unknown kind is treated as the
            // safe choice.
            _ => Self::RejectOnce,
        }
    }
}
