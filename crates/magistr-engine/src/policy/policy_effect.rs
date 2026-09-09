use serde::{Deserialize, Serialize};

/// What a policy rule does to a request that matches it, ordered from
/// most permissive to most restrictive.
///
/// The ordering is the whole mechanism. Resolution takes the *maximum*
/// effect across every matching rule, and enforcement takes the maximum
/// of that and whatever the local permission profile decided — so a
/// local config, an auto-allow rule, or even the `yolo` profile can only
/// ever be tightened by the org's policy, never loosened. Tamper
/// resistance is arithmetic rather than a check someone can forget, the
/// same way [`BudgetPolicy`] makes a spending cap arithmetic.
///
/// [`BudgetPolicy`]: crate::cost::BudgetPolicy
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    /// Nothing to say — the request passes untouched.
    #[default]
    Allow,
    /// Permitted, but recorded as a violation for the compliance report.
    Warn,
    /// A human must approve before it runs.
    RequireApproval,
    /// Refused outright.
    Deny,
}

impl PolicyEffect {
    /// Every effect, most permissive first.
    pub const ALL: [PolicyEffect; 4] = [
        PolicyEffect::Allow,
        PolicyEffect::Warn,
        PolicyEffect::RequireApproval,
        PolicyEffect::Deny,
    ];

    /// The serialized name of this effect.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyEffect::Allow => "allow",
            PolicyEffect::Warn => "warn",
            PolicyEffect::RequireApproval => "require_approval",
            PolicyEffect::Deny => "deny",
        }
    }

    /// Whether this effect is worth reporting — anything above `Allow`
    /// is a policy violation the compliance report counts.
    #[must_use]
    pub fn is_violation(self) -> bool {
        self > PolicyEffect::Allow
    }

    /// The tighter of two effects.
    #[must_use]
    pub fn tighten(self, other: PolicyEffect) -> PolicyEffect {
        self.max(other)
    }
}

impl std::fmt::Display for PolicyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PolicyEffect {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "allow" => Ok(PolicyEffect::Allow),
            "warn" => Ok(PolicyEffect::Warn),
            "require_approval" | "ask" | "approval" => Ok(PolicyEffect::RequireApproval),
            "deny" | "block" => Ok(PolicyEffect::Deny),
            other => Err(format!(
                "unknown policy effect '{other}' — valid effects: allow, warn, require_approval, deny"
            )),
        }
    }
}

#[cfg(test)]
#[path = "policy_effect.tests.rs"]
mod tests;
