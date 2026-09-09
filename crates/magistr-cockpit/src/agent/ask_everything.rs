//! The policy of no policy.

use magistr_acp::AcpPermissionRequest;
use magistr_engine::policy::RiskTier;

use super::policy_decision::PolicyDecision;
use super::session_policy::SessionPolicy;

/// A [`SessionPolicy`] that forwards every request to the cockpit —
/// what a session runs on when no profile applies.
///
/// It classifies nothing, so it answers [`RiskTier::Medium`]: a policy
/// with no opinion must not be the thing that declares a call safe, and
/// must not be the thing that declares it dangerous enough to
/// checkpoint before either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AskEverything;

impl SessionPolicy for AskEverything {
    fn decide(&mut self, _request: &AcpPermissionRequest) -> PolicyDecision {
        PolicyDecision::Ask(RiskTier::Medium)
    }
}
