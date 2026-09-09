//! What a policy says about one permission request.

use magistr_engine::policy::RiskTier;

/// The policy's answer for a permission request, decided before the
/// cockpit is involved.
///
/// `Ask` carries the risk because the policy is the only thing that has
/// classified the request — it is what decided the request would be
/// asked at all. Handing that judgement on rather than letting the
/// cockpit re-derive one from the tool's title is what stops two
/// classifiers from disagreeing about the same call; the cockpit uses
/// it to decide whether to checkpoint first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Answer the request's allow option; no strip is shown.
    Allow,
    /// Answer the request's reject option; no strip is shown.
    Deny,
    /// The policy has no say — the cockpit asks the user, and this is
    /// how dangerous the policy judged the request.
    Ask(RiskTier),
}
