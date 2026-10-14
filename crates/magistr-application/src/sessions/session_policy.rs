//! The decision a session asks for before a permission request
//! reaches the cockpit.

use super::policy_request::SessionPolicyRequest;

use super::permission_kind::PermissionKind;
use super::policy_decision::PolicyDecision;

/// What the session asks before posting a permission request to the
/// cockpit. The permission profile, the protected paths and the
/// workflow-mode rules live behind this port, outside the crate; the
/// session only routes the answer — an allow or a deny goes straight
/// back to the agent, an ask becomes the strip.
///
/// [`decide`] takes `&mut self` so a policy can keep its own records —
/// the audit log, above all. `Send + Sync` because the session's task
/// holds it across awaits.
///
/// The three other verbs are the charter's: what the reader *answered* is the policy's to
/// tally and, for a standing answer, to write as a rule; a written
/// condition is a standing order over the ask it was written on; and
/// an accepted offer widens the charter by the scope it named. Each
/// defaults to nothing, so a policy with no charter — the harness's
/// ask-everything, a fake — implements only `decide`.
///
/// [`decide`]: SessionPolicy::decide
pub trait SessionPolicy: Send + Sync {
    /// The policy's answer for `request`.
    fn decide(&mut self, request: &SessionPolicyRequest) -> PolicyDecision;

    /// Model choices obey the same frozen session policy. Policies without model
    /// restrictions permit the provider's advertised values.
    fn can_use_model(&self, _model: &str) -> bool {
        true
    }

    /// Confirmed provider state updates the model used for subsequent decisions.
    fn model_changed(&mut self, _model: Option<alloc::string::String>) {}

    /// The reader answered `request` with `kind` — tally it under the
    /// ask's scope, and write the standing rule an *always* or a
    /// *never* is.
    fn answered(&mut self, _request: &SessionPolicyRequest, _kind: PermissionKind) {}

    /// The reader wrote `condition` on `request` — the reason to ask
    /// again under this scope, recorded as a rule in their words.
    fn constrain(&mut self, _request: &SessionPolicyRequest, _condition: &str) {}

    /// The reader accepted the charter's offer for the scope `key`.
    /// `true` when a rule was written.
    fn delegate(&mut self, _key: &str) -> bool {
        false
    }
}
