//! One rule of the session's charter, as the rail shows it.

use super::rule_mode::RuleMode;

/// A charter row on the native rail: what the rule is about, in the
/// reader's words, and the mode badge it wears.
///
/// Deliberately *not* the policy engine's `PolicyRule`. The cockpit
/// shows a list of rows and never evaluates anything — the enforcement
/// lives in the session's policy, and a copy of the condition language
/// here would be a second place for the rules to live. The subject is
/// display text the seeder derives once (from a rule's id or its
/// shorthand), the way `ShipView` holds the plan's words rather than
/// the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharterRule {
    subject: String,
    mode: RuleMode,
}

impl CharterRule {
    /// A rule row: `subject` in the reader's words, wearing `mode`.
    #[must_use]
    pub fn new(subject: impl Into<String>, mode: RuleMode) -> Self {
        Self {
            subject: subject.into(),
            mode,
        }
    }

    /// What the rule is about.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The mode badge it wears.
    #[must_use]
    pub const fn mode(&self) -> RuleMode {
        self.mode
    }
}

#[cfg(test)]
#[path = "charter_rule.tests.rs"]
mod tests;
