//! Resolve a run's budget against the team's published policy.

use std::path::Path;

use chrono::{DateTime, Utc};
use magistr_engine::cost::{BudgetConfig, BudgetPolicy, ResolvedBudget};

use super::spend_by_scope::spend_by_scope;
use super::spend_context::SpendContext;
use super::storage::load_entries;
use crate::team::TeamConfig;

/// The budget a run in `dir` actually gets, given the roster's policy,
/// what the ledger at `ledger` already spent, and the local budget the
/// caller asked for.
///
/// Every read here is best-effort: an absent roster is solo mode and an
/// unreadable ledger reads as zero prior spend. Failing open on the
/// *ledger* is deliberate — it is a local file a determined developer can
/// always delete — but note what survives it: the caps themselves come
/// from the roster, so a wiped ledger widens a windowed cap without ever
/// removing one. A cap that does not depend on history, such as a
/// per-session limit, binds regardless.
#[must_use]
pub fn governed_budget(
    dir: &Path,
    ledger: &Path,
    local: &BudgetConfig,
    now: DateTime<Utc>,
) -> ResolvedBudget {
    let roster = TeamConfig::load(dir).ok().flatten();
    let policy = roster
        .as_ref()
        .map_or_else(BudgetPolicy::default, TeamConfig::policy);

    if !policy.is_set() {
        return policy.resolve(local, &Default::default());
    }

    let project = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let context = SpendContext::for_project(project).with_roster(
        roster
            .map(|r| r.members.into_iter().map(|m| m.id).collect())
            .unwrap_or_default(),
    );

    let entries = load_entries(ledger).unwrap_or_default();
    let spent = spend_by_scope(&entries, &policy, &context, now);
    policy.resolve(local, &spent)
}

#[cfg(test)]
#[path = "governed_budget.tests.rs"]
mod tests;
