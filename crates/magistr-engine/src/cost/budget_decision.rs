use serde::{Deserialize, Serialize};

use super::budget_scope::BudgetScope;
use super::resolved_budget::ResolvedBudget;
use super::scope_headroom::ScopeHeadroom;

/// A budget resolution rendered for the record.
///
/// [`ResolvedBudget`] is what the session runs under; a decision is the
/// *account* of how it got there — what the developer asked for, what
/// they were given, which scope took the difference, and what every
/// scope had left. Enforcement that cannot be reconstructed afterwards
/// is indistinguishable from no enforcement, so this exists to be
/// logged, once, at the moment the budget is decided.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetDecision {
    /// The limit the developer's own flag or config asked for.
    pub requested_limit: Option<f64>,
    /// The limit the session actually runs under.
    pub effective_limit: Option<f64>,
    /// The scope whose headroom produced the effective limit.
    pub binding: Option<BudgetScope>,
    /// Headroom for every scope carrying a limit, broadest first.
    pub headroom: Vec<ScopeHeadroom>,
}

impl BudgetDecision {
    /// The decision that produced `resolved` from `requested_limit`.
    #[must_use]
    pub fn new(requested_limit: Option<f64>, resolved: &ResolvedBudget) -> Self {
        Self {
            requested_limit,
            effective_limit: resolved.effective.limit,
            binding: resolved.binding,
            headroom: resolved.headroom.clone(),
        }
    }

    /// Whether a scope tightened what the developer asked for.
    #[must_use]
    pub const fn was_tightened(&self) -> bool {
        self.binding.is_some()
    }

    /// The binding scope's name, or `unbound` when the local budget was
    /// already the tighter of the two.
    #[must_use]
    pub fn binding_label(&self) -> &'static str {
        self.binding.as_ref().map_or("unbound", BudgetScope::as_str)
    }

    /// `scope=period:remaining/limit` for every scope, broadest first —
    /// the "what remained" half of the record, flattened so it survives
    /// a line-oriented log.
    #[must_use]
    pub fn headroom_summary(&self) -> String {
        self.headroom
            .iter()
            .map(|h| {
                format!(
                    "{}={}:{:.4}/{:.4}",
                    h.scope.as_str(),
                    h.period.as_str(),
                    h.remaining,
                    h.limit
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Write this decision to the trace.
    ///
    /// Governed runs log at `info` because the record is the point; an
    /// ungoverned run logs at `debug` so a solo developer's every
    /// invocation does not narrate a policy that does not exist.
    pub fn log(&self) {
        let requested = self.requested_limit.unwrap_or(f64::INFINITY);
        let effective = self.effective_limit.unwrap_or(f64::INFINITY);
        if self.headroom.is_empty() {
            tracing::debug!(
                requested_limit = requested,
                effective_limit = effective,
                "budget: ungoverned"
            );
            return;
        }
        tracing::info!(
            requested_limit = requested,
            effective_limit = effective,
            binding = self.binding_label(),
            tightened = self.was_tightened(),
            headroom = %self.headroom_summary(),
            "budget resolved against the team policy"
        );
    }
}

#[cfg(test)]
#[path = "budget_decision.tests.rs"]
mod tests;
