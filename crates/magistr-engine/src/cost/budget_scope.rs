use serde::{Deserialize, Serialize};

/// The level of the organization a budget rule constrains.
///
/// Ordered broadest-first: an `Org` cap covers everything a `Team` cap
/// covers, which covers a `Project`, which covers one `Session`. Budget
/// resolution walks the scopes in this order and takes the tightest
/// headroom, so a narrow scope can only ever restrict a broad one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetScope {
    /// Every session recorded in the ledger.
    Org,
    /// Sessions run by members of the team roster.
    Team,
    /// Sessions run against one project.
    Project,
    /// The session about to start.
    Session,
}

impl BudgetScope {
    /// Every scope, broadest first.
    pub const ALL: [BudgetScope; 4] = [
        BudgetScope::Org,
        BudgetScope::Team,
        BudgetScope::Project,
        BudgetScope::Session,
    ];

    /// The serialized name of this scope.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetScope::Org => "org",
            BudgetScope::Team => "team",
            BudgetScope::Project => "project",
            BudgetScope::Session => "session",
        }
    }
}

impl std::fmt::Display for BudgetScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "budget_scope.tests.rs"]
mod tests;
