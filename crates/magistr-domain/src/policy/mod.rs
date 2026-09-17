//! Workflow policies and governance — the guardrails an organization
//! publishes and every session runs under.
//!
//! Where cost governance makes a spending cap arithmetic by taking
//! the *minimum* of the local budget and every scope's headroom, this
//! module makes a guardrail arithmetic by taking the *maximum* of a
//! severity: [`PolicyEffect`] is ordered `Allow < Warn < RequireApproval
//! < Deny`, [`PolicyEngine::evaluate`] folds `max` over every matching
//! rule, and [`PolicyEngine::tighten`] folds that against whatever the
//! local permission profile decided. A developer's own config can raise
//! the floor and can never lower it — no check to forget, no rule order
//! to get wrong.
//!
//! [`GovernancePolicy`] is the roster-facing artifact: hand-written
//! `rules` plus shorthands for the four things every team wants
//! (tool allow/denylist, protected paths, approved models), all
//! desugared by [`GovernancePolicy::compile`] into one engine.
//!
//! Under the roster sits the reader's own [`Charter`]: the delegations
//! they wrote by answering asks, applied to the *local* decision before
//! the roster tightens it, and the per-scope tally of their answers the
//! charter's offers are made from. A charter widens what the profile
//! would ask about; it can never widen what the roster refuses.

pub mod charter;
pub mod delegation_offer;
pub mod glob_match;
pub mod governance_policy;
pub mod model_tier;
pub mod model_tier_rule;
pub mod policy_condition;
pub mod policy_decision;
pub mod policy_effect;
pub mod policy_engine;
pub mod policy_request;
pub mod policy_rule;
pub mod risk_tier;
pub mod scope_tally;

pub use charter::Charter;
pub use delegation_offer::DelegationOffer;
pub use glob_match::glob_match;
pub use governance_policy::GovernancePolicy;
pub use model_tier::ModelTier;
pub use model_tier_rule::ModelTierRule;
pub use policy_condition::PolicyCondition;
pub use policy_decision::PolicyDecision;
pub use policy_effect::PolicyEffect;
pub use policy_engine::PolicyEngine;
pub use policy_request::PolicyRequest;
pub use policy_rule::PolicyRule;
pub use risk_tier::RiskTier;
pub use scope_tally::ScopeTally;

// The permission decision is one cohesive module behind this facade: the
// context a host observes, the service that decides, and the resolution with
// its outcome and source change together, so they live together.
mod permission;
pub use permission::{
    PermissionContext, PermissionMode, PermissionOutcome, PermissionPolicyService,
    PermissionResolution, PermissionSource,
};
