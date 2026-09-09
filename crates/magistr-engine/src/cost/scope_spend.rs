use std::collections::BTreeMap;

use super::budget_period::BudgetPeriod;
use super::budget_scope::BudgetScope;

/// What each `(scope, window)` pair has already spent.
///
/// Keyed by the pair rather than the scope alone because one scope may
/// carry several rules — a team with both a daily and a monthly cap
/// measures its spend over two different windows.
pub type ScopeSpend = BTreeMap<(BudgetScope, BudgetPeriod), f64>;
