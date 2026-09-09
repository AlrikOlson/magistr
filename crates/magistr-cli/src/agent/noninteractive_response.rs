//! Map shared policy decisions to ACP choices for a session with no approval UI.

use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionRequest, AcpPermissionResponse};
use magistr_cockpit::agent::{PolicyDecision, SessionPolicy};

use crate::permissions::NoninteractivePolicy;

/// Resolve a request without weakening policy Deny or treating rejection as allowance.
pub fn noninteractive_response(
    policy: &mut dyn SessionPolicy,
    request: &AcpPermissionRequest,
    handling: NoninteractivePolicy,
) -> AcpPermissionResponse {
    let once = |kind| request.options.iter().find(|option| option.kind == kind);
    let choice = match policy.decide(request) {
        PolicyDecision::Allow => once(Kind::AllowOnce),
        PolicyDecision::Deny => once(Kind::RejectOnce),
        PolicyDecision::Ask(_) => match handling {
            NoninteractivePolicy::Refuse => None,
            NoninteractivePolicy::LegacyAutomaticApproval => once(Kind::AllowOnce),
        },
    };
    choice.map_or(AcpPermissionResponse::Cancelled, |option| {
        AcpPermissionResponse::Selected(option.option_id.clone())
    })
}
