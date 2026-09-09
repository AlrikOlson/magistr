//! Compliance reporting over the permission audit log.

use std::collections::BTreeMap;

use super::audit_entry::AuditEntry;

/// The prefix [`DecisionReason::GovernancePolicy`] writes into the log.
///
/// [`DecisionReason::GovernancePolicy`]: super::decision_reason::DecisionReason::GovernancePolicy
const REASON_PREFIX: &str = "policy rule: ";

/// What the org's guardrails actually caught, read back off the audit log.
///
/// The report is derived rather than separately recorded: enforcement
/// already writes the binding rule's id into the existing audit log, so
/// there is no second store to keep in sync and no way for the report to
/// disagree with what was enforced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViolationReport {
    /// How many audit entries a policy rule decided.
    pub total: usize,
    /// Violations per rule id, rule-id order.
    pub by_rule: BTreeMap<String, usize>,
    /// Violations per actor, actor order; unattributed entries group
    /// under `unattributed` rather than vanishing.
    pub by_actor: BTreeMap<String, usize>,
    /// How many of those were refused outright.
    pub denied: usize,
}

impl ViolationReport {
    /// The key unattributed entries group under.
    pub const UNATTRIBUTED: &'static str = "unattributed";

    /// Summarize the policy-decided entries in `entries`.
    #[must_use]
    pub fn from_entries(entries: &[AuditEntry]) -> Self {
        let mut report = Self::default();
        for entry in entries {
            let Some(rule) = entry.reason.strip_prefix(REASON_PREFIX) else {
                continue;
            };
            report.total += 1;
            if !entry.allowed {
                report.denied += 1;
            }
            *report.by_rule.entry(rule.to_string()).or_default() += 1;
            let actor = entry
                .actor
                .clone()
                .unwrap_or_else(|| Self::UNATTRIBUTED.to_string());
            *report.by_actor.entry(actor).or_default() += 1;
        }
        report
    }

    /// Whether the log recorded any policy decision at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Rules by violation count, most-tripped first, ties by rule id.
    #[must_use]
    pub fn ranked_rules(&self) -> Vec<(&str, usize)> {
        let mut rows: Vec<(&str, usize)> = self
            .by_rule
            .iter()
            .map(|(rule, count)| (rule.as_str(), *count))
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        rows
    }
}

#[cfg(test)]
#[path = "violation_report.tests.rs"]
mod tests;
