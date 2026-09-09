use serde::{Deserialize, Serialize};

use super::model_tier::ModelTier;
use super::model_tier_rule::ModelTierRule;
use super::policy_condition::PolicyCondition;
use super::policy_effect::PolicyEffect;
use super::policy_engine::PolicyEngine;
use super::policy_rule::PolicyRule;

/// The governance policy an organization publishes in its team roster.
///
/// Two ways in, one way out. A team can hand-write `rules` for anything
/// the condition language expresses, and it can use the shorthands —
/// `tool_allowlist`, `tool_denylist`, `protected_paths`, `approved_models`
/// — for the four things every team wants. [`compile`] desugars the
/// shorthands into ordinary [`PolicyRule`]s and returns a single
/// [`PolicyEngine`], so there is exactly one evaluation path and no
/// second mechanism that can disagree with the first.
///
/// The gate half (`required_gates`, `allow_gate_bypass`) is not a
/// per-request question — it asks whether a *set* of gates ran — so it
/// stays as [`missing_required_gates`] rather than being forced through
/// the engine.
///
/// [`compile`]: GovernancePolicy::compile
/// [`missing_required_gates`]: GovernancePolicy::missing_required_gates
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernancePolicy {
    /// Hand-written rules, in full generality.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,

    /// Globs for the only tools permitted. Empty means "no allowlist";
    /// non-empty denies every tool that matches none of them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_allowlist: Vec<String>,

    /// Globs for tools that are refused outright.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_denylist: Vec<String>,

    /// Globs for paths an agent may not write without a human saying so
    /// on the record. Denied-but-overridable, never silently allowed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_paths: Vec<String>,

    /// Gate names that must have passed before work counts as done.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_gates: Vec<String>,

    /// Whether a developer may ship past a missing required gate.
    #[serde(default)]
    pub allow_gate_bypass: bool,

    /// Globs for the only models permitted. Empty means "no allowlist".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approved_models: Vec<String>,

    /// How model families map onto cost tiers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_tiers: Vec<ModelTierRule>,

    /// The most expensive tier this policy permits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_model_tier: Option<ModelTier>,

    /// Where to route a session whose requested model is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_model: Option<String>,
}

impl GovernancePolicy {
    /// Whether this policy says anything at all.
    #[must_use]
    pub fn is_set(&self) -> bool {
        !self.rules.is_empty()
            || !self.tool_allowlist.is_empty()
            || !self.tool_denylist.is_empty()
            || !self.protected_paths.is_empty()
            || !self.required_gates.is_empty()
            || !self.approved_models.is_empty()
            || self.max_model_tier.is_some()
    }

    /// Whether this policy says nothing at all — the serde predicate
    /// that keeps an ungoverned roster free of an empty `policy` block.
    #[must_use]
    pub fn is_unset(&self) -> bool {
        !self.is_set() && !self.allow_gate_bypass && self.fallback_model.is_none()
    }

    /// Desugar the shorthands and return the one engine that decides.
    #[must_use]
    pub fn compile(&self) -> PolicyEngine {
        let mut rules = self.rules.clone();

        for pattern in &self.tool_denylist {
            rules.push(
                PolicyRule::new(
                    format!("tool-denylist:{pattern}"),
                    PolicyCondition::tool(pattern),
                    PolicyEffect::Deny,
                )
                .describe(format!("tool '{pattern}' is on the org denylist")),
            );
        }

        if !self.tool_allowlist.is_empty() {
            rules.push(
                PolicyRule::new(
                    "tool-allowlist",
                    Self::not_on_list(
                        PolicyCondition::tool("*"),
                        self.tool_allowlist.iter().map(PolicyCondition::tool),
                    ),
                    PolicyEffect::Deny,
                )
                .describe(format!(
                    "only these tools are approved: {}",
                    self.tool_allowlist.join(", ")
                )),
            );
        }

        for pattern in &self.protected_paths {
            rules.push(
                PolicyRule::new(
                    format!("protected-path:{pattern}"),
                    PolicyCondition::path(pattern),
                    PolicyEffect::Deny,
                )
                .describe(format!("'{pattern}' is a protected path"))
                .overridable(),
            );
        }

        if !self.approved_models.is_empty() {
            rules.push(
                PolicyRule::new(
                    "model-allowlist",
                    Self::not_on_list(
                        PolicyCondition::model("*"),
                        self.approved_models.iter().map(PolicyCondition::model),
                    ),
                    PolicyEffect::Deny,
                )
                .describe(format!(
                    "only these models are approved: {}",
                    self.approved_models.join(", ")
                )),
            );
        }

        if let Some(max) = self.max_model_tier {
            for tier_rule in self.model_tiers.iter().filter(|r| r.tier > max) {
                rules.push(
                    PolicyRule::new(
                        format!("model-tier:{}", tier_rule.pattern),
                        PolicyCondition::model(&tier_rule.pattern),
                        PolicyEffect::Deny,
                    )
                    .describe(format!(
                        "'{}' is {} tier; this policy caps models at {max}",
                        tier_rule.pattern, tier_rule.tier
                    )),
                );
            }
        }

        PolicyEngine::new(rules)
    }

    /// The required gates that are not in `passed`, in policy order.
    ///
    /// A gate the project never configured reports as passed elsewhere in
    /// the system ("skipped, no command"), which is exactly the hole this
    /// closes: the caller passes only the gates that actually ran green.
    #[must_use]
    pub fn missing_required_gates<S: AsRef<str>>(&self, passed: &[S]) -> Vec<String> {
        self.required_gates
            .iter()
            .filter(|required| {
                !passed
                    .iter()
                    .any(|p| p.as_ref().eq_ignore_ascii_case(required))
            })
            .cloned()
            .collect()
    }

    /// The cost tier `model` falls in, when the policy classifies it.
    #[must_use]
    pub fn tier_of(&self, model: &str) -> Option<ModelTier> {
        self.model_tiers
            .iter()
            .find(|rule| rule.matches(model))
            .map(|rule| rule.tier)
    }

    /// Which model a session asking for `requested` should actually run.
    ///
    /// `Some(requested)` when it is approved, `Some(fallback)` when it is
    /// refused but the policy names somewhere to route to, and `None`
    /// when it is refused with nowhere to go.
    #[must_use]
    pub fn route_model<'a>(&'a self, requested: &'a str) -> Option<&'a str> {
        let engine = self.compile();
        let request = super::policy_request::PolicyRequest::new().with_model(requested);
        if !engine.evaluate(&request).effect.is_violation() {
            return Some(requested);
        }
        self.fallback_model.as_deref().filter(|fallback| {
            let fallback_request =
                super::policy_request::PolicyRequest::new().with_model(*fallback);
            !engine.evaluate(&fallback_request).effect.is_violation()
        })
    }

    /// `present AND NOT (any of `listed`)` — the allowlist shape.
    fn not_on_list(
        present: PolicyCondition,
        listed: impl Iterator<Item = PolicyCondition>,
    ) -> PolicyCondition {
        PolicyCondition::all_of(vec![
            present,
            PolicyCondition::negate(PolicyCondition::any_of(listed.collect())),
        ])
    }
}

#[cfg(test)]
#[path = "governance_policy.tests.rs"]
mod tests;
