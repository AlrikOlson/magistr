use alloc::{format, string::ToString};

use super::{
    Charter, PermissionContext, PermissionMode, PermissionOutcome, PermissionResolution,
    PermissionSource, PolicyEffect, PolicyEngine, PolicyRequest, RiskTier,
};

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
#[path = "permission_policy_service.tests.rs"]
mod tests;
