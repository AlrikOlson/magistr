//! What each `(scope, window)` in a policy has already spent.

use chrono::{DateTime, Utc};
use magistr_engine::cost::{BudgetPolicy, ScopeSpend};

use super::cost_entry::CostEntry;
use super::spend_context::SpendContext;

/// Total the ledger against every `(scope, window)` pair the policy
/// actually constrains.
///
/// Only pairs belonging to a rule that sets a threshold are computed —
/// an inert rule costs no work — and a `Session` window contributes zero
/// by construction, since the session has not started.
#[must_use]
pub fn spend_by_scope(
    entries: &[CostEntry],
    policy: &BudgetPolicy,
    context: &SpendContext,
    now: DateTime<Utc>,
) -> ScopeSpend {
    let mut spend = ScopeSpend::new();

    for rule in &policy.rules {
        if rule.limit.is_none() && rule.warn_threshold.is_none() {
            continue;
        }
        let key = (rule.scope, rule.period);
        if spend.contains_key(&key) {
            continue;
        }
        let since = rule.period.window_start(now);
        let total = entries
            .iter()
            .filter(|e| context.in_scope(e, rule.scope))
            .filter(|e| since.is_none_or(|start| e.timestamp >= start))
            .map(|e| e.cost_usd)
            .sum();
        spend.insert(key, total);
    }

    spend
}

#[cfg(test)]
#[path = "spend_by_scope.tests.rs"]
mod tests;
