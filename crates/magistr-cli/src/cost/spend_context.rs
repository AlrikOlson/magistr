//! Which ledger entries a budget scope is measured over.

use magistr_engine::cost::BudgetScope;

use super::cost_entry::CostEntry;

/// What the current run is, so a scope can decide which past spend
/// counts against it.
///
/// A file-backed ledger cannot tell an org from a team on its own, so
/// the two are distinguished by the roster: `Org` is every entry,
/// `Team` is every entry whose actor is on the roster (or everything
/// when no roster exists). Entries with no actor at all — everything
/// without attribution — count against the team rather than being
/// excluded, because excluding them would hand a team unlimited
/// historical headroom the moment it adopted a cap.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpendContext {
    /// The project this run is against.
    pub project: String,
    /// The roster's member ids, empty when the project runs solo.
    pub roster: Vec<String>,
}

impl SpendContext {
    /// A context for a project with no roster.
    #[must_use]
    pub fn for_project(project: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            roster: Vec::new(),
        }
    }

    /// Attach the roster's member ids (self-consuming builder).
    #[must_use]
    pub fn with_roster(mut self, roster: Vec<String>) -> Self {
        self.roster = roster;
        self
    }

    /// Whether an entry's spend counts against `scope`.
    #[must_use]
    pub fn in_scope(&self, entry: &CostEntry, scope: BudgetScope) -> bool {
        match scope {
            BudgetScope::Org => true,
            BudgetScope::Team => match entry.actor.as_deref() {
                None => true,
                Some(actor) => {
                    self.roster.is_empty()
                        || self.roster.iter().any(|m| m.eq_ignore_ascii_case(actor))
                }
            },
            BudgetScope::Project => entry.project.eq_ignore_ascii_case(&self.project),
            // The session about to start has spent nothing yet.
            BudgetScope::Session => false,
        }
    }
}

#[cfg(test)]
#[path = "spend_context.tests.rs"]
mod tests;
