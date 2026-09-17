//! One permission decision, reached through the `policy` facade.
//!
//! A host observes a request and normalizes it into a [`PermissionContext`]
//! (its [`PermissionMode`] and the local allowances); the deterministic
//! [`PermissionPolicyService`] folds the charter and governance over it and
//! answers a [`PermissionResolution`], whose [`PermissionOutcome`] is what the
//! responder does and whose [`PermissionSource`] names the layer that decided.
//! These change together: a new source is produced by the service and carried
//! in the resolution; a new observation is a context field the service reads.
//! Classification, path resolution and configuration loading belong to the
//! host adapter, not here.

use alloc::{
    format,
    string::{String, ToString},
};

use serde::{Deserialize, Serialize};

use super::{Charter, PolicyEffect, PolicyEngine, PolicyRequest, RiskTier};

/// Explicit local allowance mode; neither mode bypasses remembered or organization rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// Apply profile defaults and the existing safe-operation allowances.
    Standard,
    /// Also allow the configured workflow's standard development operations.
    Workflow,
}

/// The authorization outcome, independent of any UI or transport response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionOutcome {
    /// The request may proceed.
    Allow,
    /// The request must be refused.
    Deny,
    /// A responder must explicitly resolve the request.
    Ask,
}

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

/// Normalized observations supplied by a host to the deterministic policy service.
/// Classification, path resolution and configuration loading belong to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionContext {
    pub mode: PermissionMode,
    pub profile_effect: PolicyEffect,
    pub profile_reason: String,
    pub local_allowance: bool,
    pub workflow_allowance: bool,
    /// A protected-target refusal is an immutable floor.
    pub protected: bool,
    /// Exact request identity including project, model and operation context.
    pub scope: String,
    /// Host-supplied identity of every behavior-affecting policy input.
    pub policy_version: u64,
}

/// One complete policy result, usable by every permission responder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionResolution {
    pub outcome: PermissionOutcome,
    pub risk: RiskTier,
    pub reason: String,
    pub source: PermissionSource,
    pub scope: String,
    pub policy_version: u64,
}

/// The common authorization contract. Hosts provide normalized observations;
/// this service combines local allowances, standing answers and governance.
/// It performs no I/O and does not claim operating-system sandbox enforcement.
pub struct PermissionPolicyService;

impl PermissionPolicyService {
    #[must_use]
    pub fn evaluate(
        request: &PolicyRequest,
        context: &PermissionContext,
        charter: &Charter,
        governance: &PolicyEngine,
    ) -> PermissionResolution {
        let (local, mut source, mut reason) = if context.profile_effect == PolicyEffect::Deny {
            (
                PolicyEffect::Deny,
                PermissionSource::Profile,
                context.profile_reason.clone(),
            )
        } else if context.mode == PermissionMode::Workflow && context.workflow_allowance {
            (
                PolicyEffect::Allow,
                PermissionSource::WorkflowAllowance,
                "workflow allowance".to_string(),
            )
        } else if context.local_allowance {
            (
                PolicyEffect::Allow,
                PermissionSource::LocalAllowance,
                "local operation allowance".to_string(),
            )
        } else {
            (
                context.profile_effect,
                PermissionSource::Profile,
                context.profile_reason.clone(),
            )
        };
        let standing = charter.apply(local, request);
        let mut effect = if local == PolicyEffect::Deny {
            local
        } else {
            standing.effect
        };
        if local != PolicyEffect::Deny
            && let Some(rule) = standing.binding
        {
            reason = format!("delegated: {rule}");
            source = PermissionSource::Charter(rule);
        }
        let governed = governance.evaluate(request);
        if governed.effect >= effect
            && let Some(rule) = governed.binding
        {
            reason = format!("policy rule: {rule}");
            source = PermissionSource::Governance(rule);
        }
        effect = effect.tighten(governed.effect);
        if context.protected {
            effect = PolicyEffect::Deny;
            source = PermissionSource::ProtectedPath;
            reason = "protected target".to_string();
        }
        PermissionResolution {
            outcome: match effect {
                PolicyEffect::Allow | PolicyEffect::Warn => PermissionOutcome::Allow,
                PolicyEffect::RequireApproval => PermissionOutcome::Ask,
                PolicyEffect::Deny => PermissionOutcome::Deny,
            },
            risk: request.risk.unwrap_or(RiskTier::Medium),
            reason,
            source,
            scope: context.scope.clone(),
            policy_version: context.policy_version,
        }
    }
}

#[cfg(test)]
#[path = "permission.tests.rs"]
mod tests;
