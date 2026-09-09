//! Compatibility exports for the pure domain policy model.

pub use magistr_domain::policy::{
    Charter, DelegationOffer, GovernancePolicy, ModelTier, ModelTierRule, PolicyCondition,
    PolicyDecision, PolicyEffect, PolicyEngine, PolicyRequest, PolicyRule, RiskTier, ScopeTally,
    charter, delegation_offer, glob_match, governance_policy, model_tier, model_tier_rule,
    policy_condition, policy_decision, policy_effect, policy_engine, policy_request, policy_rule,
    risk_tier, scope_tally,
};

pub use magistr_domain::policy::{
    PermissionContext, PermissionMode, PermissionOutcome, PermissionPolicyService,
    PermissionResolution, PermissionSource,
};
