//! One choice in a permission request.

use agent_client_protocol::schema::v1::PermissionOption;

use super::AcpPermissionOptionKind;

/// A simplified permission option from the protocol.
#[derive(Debug, Clone)]
pub struct AcpPermissionOption {
    /// Id sent back in the response.
    pub option_id: String,
    /// What choosing it means.
    pub kind: AcpPermissionOptionKind,
}

impl From<&PermissionOption> for AcpPermissionOption {
    fn from(opt: &PermissionOption) -> Self {
        Self {
            option_id: opt.option_id.to_string(),
            kind: AcpPermissionOptionKind::from(opt.kind),
        }
    }
}
