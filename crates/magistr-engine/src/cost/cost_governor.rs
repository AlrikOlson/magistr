use serde::{Deserialize, Serialize};

use crate::session::SessionCost;

use super::budget_config::BudgetConfig;
use super::budget_status::{BudgetStatus, check_budget};
use super::cost_attribution::CostAttribution;
use super::cost_event::CostEvent;
use super::cost_sample::CostSample;

/// Domain service for cost tracking and budget enforcement.
///
/// Owns session cost state and budget configuration. Evaluates budget
/// policy after each recorded turn and produces `CostEvent`s.
/// Pure domain logic — no IO, no persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostGovernor {
    /// Budget configuration.
    budget: BudgetConfig,
    /// Accumulated session cost.
    cost: SessionCost,
    /// Whether a budget warning has already been emitted.
    warning_emitted: bool,
    /// Who and what this session's spend is charged to. Every sample
    /// the governor emits carries a clone of it, which is what turns
    /// the cost channel from a total into a set of labelled series.
    #[serde(default)]
    attribution: CostAttribution,
}

impl CostGovernor {
    /// Create a new cost governor with the given budget configuration.
    #[must_use]
    pub fn new(budget: BudgetConfig) -> Self {
        Self {
            budget,
            cost: SessionCost::default(),
            warning_emitted: false,
            attribution: CostAttribution::default(),
        }
    }

    /// Supply already observed spend when constructing a value checkpoint.
    /// Live captures preserve the entire governor, including its warning flag.
    #[must_use]
    pub fn with_cost(mut self, cost: SessionCost) -> Self {
        self.cost = cost;
        self.warning_emitted = self.check_budget_status().should_warn();
        self
    }

    /// Refuse restoration that erases spend or weakens a live owner's budget.
    #[must_use]
    pub fn preserves_observations(&self, known: &Self) -> bool {
        self.cost.total_usd >= known.cost.total_usd
            && self.cost.input_tokens >= known.cost.input_tokens
            && self.cost.output_tokens >= known.cost.output_tokens
            && self.cost.turns >= known.cost.turns
            && (!known.warning_emitted || self.warning_emitted)
            && preserves_limit(self.budget.limit, known.budget.limit)
            && preserves_limit(self.budget.warn_threshold, known.budget.warn_threshold)
            && preserves_limit(self.budget.max_iterations, known.budget.max_iterations)
            && preserves_limit(
                self.budget.max_duration_secs,
                known.budget.max_duration_secs,
            )
    }

    /// Charge this session's spend to an attribution (self-consuming
    /// builder).
    #[must_use]
    pub fn with_attribution(mut self, attribution: CostAttribution) -> Self {
        self.attribution = attribution;
        self
    }

    /// Replace the attribution mid-session — the model is not always
    /// known before the agent reports it.
    pub fn set_attribution(&mut self, attribution: CostAttribution) {
        self.attribution = attribution;
    }

    // ── Accessors ───────────────────────────────────────────────────────

    /// Current budget configuration.
    #[must_use]
    pub fn budget(&self) -> &BudgetConfig {
        &self.budget
    }

    /// Current accumulated cost.
    #[must_use]
    pub fn cost(&self) -> &SessionCost {
        &self.cost
    }

    /// Who and what this session's spend is charged to.
    #[must_use]
    pub fn attribution(&self) -> &CostAttribution {
        &self.attribution
    }

    /// Total cost in USD.
    #[must_use]
    pub fn total_cost_usd(&self) -> f64 {
        self.cost.total_usd
    }

    /// Whether the budget has been exceeded.
    #[must_use]
    pub fn is_exceeded(&self) -> bool {
        self.check_budget_status().should_stop()
    }

    // ── Commands ────────────────────────────────────────────────────────

    /// Record a completed turn and return any cost events.
    ///
    /// Checks budget constraints after recording. Returns events that
    /// should be forwarded to consumers (warning issued once, exceeded
    /// every time).
    pub fn record_turn(
        &mut self,
        cost_usd: f64,
        input_tokens: u64,
        output_tokens: u64,
    ) -> Vec<CostEvent> {
        self.cost.record_turn(cost_usd, input_tokens, output_tokens);

        let mut events = vec![CostEvent::CostUpdated {
            total_cost_usd: self.cost.total_usd,
            turn_cost_usd: cost_usd,
            turns: self.cost.turns,
        }];

        let sample = CostSample::new(cost_usd, input_tokens, output_tokens)
            .with_total(self.cost.total_usd)
            .with_attribution(self.attribution.clone());
        if !sample.is_empty() {
            events.push(CostEvent::CostSampled { sample });
        }

        match self.check_budget_status() {
            BudgetStatus::Warning {
                remaining,
                threshold,
            } if !self.warning_emitted => {
                self.warning_emitted = true;
                events.push(CostEvent::BudgetWarning {
                    remaining,
                    threshold,
                    total_cost_usd: self.cost.total_usd,
                });
            }
            BudgetStatus::Exceeded { overage, limit } => {
                events.push(CostEvent::BudgetExceeded {
                    overage,
                    limit,
                    total_cost_usd: self.cost.total_usd,
                });
            }
            _ => {}
        }

        events
    }

    /// Extend (raise) the budget limit. Returns a `BudgetExtended` event.
    ///
    /// Returns `None` if no limit was previously set.
    pub fn extend_budget(&mut self, new_limit: f64) -> Option<CostEvent> {
        let old_limit = self.budget.limit?;
        self.budget.limit = Some(new_limit);
        // Reset warning flag if new limit moves us back under threshold
        if !self.check_budget_status().should_warn() {
            self.warning_emitted = false;
        }
        Some(CostEvent::BudgetExtended {
            old_limit,
            new_limit,
        })
    }

    // ── Internal ────────────────────────────────────────────────────────

    /// Check current spending against budget configuration.
    fn check_budget_status(&self) -> BudgetStatus {
        check_budget(self.cost.total_usd, &self.budget)
    }
}

fn preserves_limit<T: PartialOrd>(incoming: Option<T>, known: Option<T>) -> bool {
    match (incoming, known) {
        (_, None) => true,
        (Some(incoming), Some(known)) => incoming <= known,
        (None, Some(_)) => false,
    }
}

#[cfg(test)]
#[path = "cost_governor.tests.rs"]
mod tests;
