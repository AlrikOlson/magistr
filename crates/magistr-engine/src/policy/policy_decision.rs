use super::policy_effect::PolicyEffect;
use super::policy_rule::PolicyRule;

/// What a [`PolicyEngine`] concluded about one request.
///
/// `matched` lists every rule that fired, not just the one that set the
/// effect — a compliance report wants all of them, and the audit log
/// wants to name what a developer actually tripped.
///
/// [`PolicyEngine`]: super::policy_engine::PolicyEngine
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyDecision {
    /// The tightest effect across every matching rule.
    pub effect: PolicyEffect,
    /// The ids of every rule that matched, in rule order.
    pub matched: Vec<String>,
    /// The id of the rule that set `effect` (the first at that height).
    pub binding: Option<String>,
    /// Whether every rule at the binding effect allows an override.
    pub override_allowed: bool,
}

impl PolicyDecision {
    /// The decision for a request no rule matched.
    #[must_use]
    pub fn unconstrained() -> Self {
        Self::default()
    }

    /// Fold the matching rules into a decision: the effect is the
    /// *maximum* over them, so adding a rule can only ever tighten.
    #[must_use]
    pub fn from_matches<'a>(matches: impl IntoIterator<Item = &'a PolicyRule>) -> Self {
        let mut decision = Self::unconstrained();
        let mut binding_rules: Vec<&PolicyRule> = Vec::new();

        for rule in matches {
            decision.matched.push(rule.id.clone());
            match rule.effect.cmp(&decision.effect) {
                std::cmp::Ordering::Greater => {
                    decision.effect = rule.effect;
                    decision.binding = Some(rule.id.clone());
                    binding_rules.clear();
                    binding_rules.push(rule);
                }
                std::cmp::Ordering::Equal if decision.effect.is_violation() => {
                    binding_rules.push(rule);
                }
                _ => {}
            }
        }

        // An override is only on the table when *every* rule holding the
        // decision at its height permits one — otherwise the strictest
        // rule would be silently overridable through a laxer sibling.
        decision.override_allowed =
            !binding_rules.is_empty() && binding_rules.iter().all(|r| r.override_allowed);
        decision
    }

    /// Whether this decision is worth reporting.
    #[must_use]
    pub fn is_violation(&self) -> bool {
        self.effect.is_violation()
    }

    /// A one-line explanation naming the binding rule.
    #[must_use]
    pub fn explain(&self) -> String {
        match &self.binding {
            Some(id) => format!("{} by policy rule '{id}'", self.effect),
            None => "no policy rule applies".to_string(),
        }
    }
}

#[cfg(test)]
#[path = "policy_decision.tests.rs"]
mod tests;
