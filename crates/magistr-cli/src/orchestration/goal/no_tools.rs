//! The judge's permission policy: nothing is ever allowed.

use magistr_application::sessions::SessionPolicyRequest;
use magistr_application::sessions::{PolicyDecision, SessionPolicy};

/// Denies every permission request. The judge holds no tools whatever
/// it asks for, so its session owner answers each ask with the reject
/// option — never an allow, never a question for a reader who is not
/// there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoTools;

impl SessionPolicy for NoTools {
    fn decide(&mut self, _request: &SessionPolicyRequest) -> PolicyDecision {
        PolicyDecision::Deny
    }
}
