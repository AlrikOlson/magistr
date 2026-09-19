//! Cost tracking and budget management.

// ── Core types ─────────────────────────────────────────────────
pub mod budget_config;
pub mod cost_entry;
pub mod cost_summary;
pub mod types;

// ── Metering & chargeback ──────────────────────────────────────
pub mod chargeback;
pub mod chargeback_row;
pub mod cost_dimension;
pub mod cost_group;
pub mod summarize_by_dimension;

// ── Budget ─────────────────────────────────────────────────────
pub mod budget;
pub mod check_budget;

// ── Hierarchical governance (org → team → project → session) ────
pub mod governed_budget;
pub mod spend_by_scope;
pub mod spend_context;

// ── Estimation ─────────────────────────────────────────────────
pub mod estimate;
pub mod estimate_cost;
pub mod estimate_turns;

// ── Budget exceeded screen ─────────────────────────────────────
pub mod exceeded;

// ── Optimization ───────────────────────────────────────────────
pub mod bloated_file;
pub mod collect_hints;
pub mod context_bloat_analysis;
pub mod detect_context_bloat;
pub mod detect_retry_loop;
pub mod hint_severity;
pub mod optimization;
pub mod pruning_suggestion;
pub mod retry_loop_analysis;
pub mod retry_loop_hint;
pub mod turn_outcome;

// ── Storage ────────────────────────────────────────────────────
pub mod append_entry;
pub mod costs_path;
pub mod csv_export;
pub mod filter_by_project;
pub mod filter_since;
pub mod record_session;
pub mod record_session_at;
pub mod session_spend;
pub mod storage;
pub mod summarize_by_period;
pub mod summarize_costs;
pub mod time_bucket;
pub mod time_period;

// ── Re-exports ─────────────────────────────────────────────────
#[allow(unused_imports)]
pub use bloated_file::BloatedFile;
#[allow(unused_imports)]
pub use budget::BudgetStatus;
#[allow(unused_imports)]
pub use budget_config::BudgetConfig;
#[allow(unused_imports)]
pub use chargeback::chargeback;
#[allow(unused_imports)]
pub use chargeback_row::ChargebackRow;
#[allow(unused_imports)]
pub use check_budget::check_budget;
#[allow(unused_imports)]
pub use collect_hints::collect_hints;
#[allow(unused_imports)]
pub use context_bloat_analysis::ContextBloatAnalysis;
#[allow(unused_imports)]
pub use cost_dimension::CostDimension;
#[allow(unused_imports)]
pub use cost_entry::CostEntry;
#[allow(unused_imports)]
pub use cost_group::CostGroup;
#[allow(unused_imports)]
pub use cost_summary::CostSummary;
#[allow(unused_imports)]
pub use detect_context_bloat::detect_context_bloat;
#[allow(unused_imports)]
pub use detect_retry_loop::detect_retry_loop;
#[allow(unused_imports)]
pub use exceeded::BudgetExceededScreen;
#[allow(unused_imports)]
pub use governed_budget::governed_budget;
#[allow(unused_imports)]
pub use hint_severity::HintSeverity;
#[allow(unused_imports)]
pub use optimization::OptimizationHint;
#[allow(unused_imports)]
pub use pruning_suggestion::pruning_suggestion;
#[allow(unused_imports)]
pub use record_session::record_session;
#[allow(unused_imports)]
pub use retry_loop_analysis::RetryLoopAnalysis;
#[allow(unused_imports)]
pub use retry_loop_hint::retry_loop_hint;
#[allow(unused_imports)]
pub use session_spend::SessionSpend;
#[allow(unused_imports)]
pub use spend_by_scope::spend_by_scope;
#[allow(unused_imports)]
pub use spend_context::SpendContext;
#[allow(unused_imports)]
pub use summarize_by_dimension::summarize_by_dimension;
#[allow(unused_imports)]
pub use turn_outcome::TurnOutcome;
#[allow(unused_imports)]
pub use types::TokenBreakdown;

use chrono::Utc;

/// Per-session cost tracker with budget enforcement.
///
/// Accumulates cost and token usage across agent turns.
/// Checks budget constraints after each turn.
#[derive(Debug)]
pub struct CostTracker {
    /// Budget configuration for this session.
    budget: BudgetConfig,
    /// Accumulated cost in USD.
    accumulated_cost: f64,
    /// Accumulated token usage.
    accumulated_tokens: TokenBreakdown,
    /// Total turns recorded.
    total_turns: u32,
    /// Total duration in ms.
    total_duration_ms: u64,
    /// Per-turn cost history (for sparklines).
    turn_costs: Vec<f64>,
    /// Whether a budget warning has already been issued this session.
    warning_issued: bool,
    /// The team identity this session's spend is charged to.
    actor: Option<String>,
    /// The model the session is running on.
    model: Option<String>,
}

impl CostTracker {
    /// Create a new tracker with the given budget configuration.
    pub fn new(budget: BudgetConfig) -> Self {
        Self {
            budget,
            accumulated_cost: 0.0,
            accumulated_tokens: TokenBreakdown::default(),
            total_turns: 0,
            total_duration_ms: 0,
            turn_costs: Vec::new(),
            warning_issued: false,
            actor: None,
            model: None,
        }
    }

    /// Charge this session's spend to a team identity (self-consuming builder).
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Attribute this session's spend to a model (self-consuming builder).
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Record the model once the agent reports it mid-session.
    pub fn set_model(&mut self, model: impl Into<String>) {
        self.model = Some(model.into());
    }

    /// The identity this session's spend is charged to.
    pub fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }

    /// The model this session is running on.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Record a completed turn and return the budget status.
    ///
    /// If the budget warning threshold is crossed for the first time,
    /// returns `Warning`. Once issued, subsequent calls return `Ok`
    /// until the hard limit is reached.
    pub fn record_turn(
        &mut self,
        cost_usd: f64,
        duration_ms: u64,
        tokens: Option<&TokenBreakdown>,
    ) -> BudgetStatus {
        self.accumulated_cost += cost_usd;
        self.total_duration_ms += duration_ms;
        self.total_turns += 1;
        self.turn_costs.push(cost_usd);

        if let Some(t) = tokens {
            self.accumulated_tokens.accumulate(t);
        }

        let status = check_budget::check_budget(self.accumulated_cost, &self.budget);

        // Only issue warning once
        match &status {
            BudgetStatus::Warning { .. } if self.warning_issued => BudgetStatus::Ok,
            BudgetStatus::Warning { .. } => {
                self.warning_issued = true;
                status
            }
            _ => status,
        }
    }

    /// Current accumulated cost.
    pub fn total_cost(&self) -> f64 {
        self.accumulated_cost
    }

    /// Current accumulated tokens.
    pub fn total_tokens(&self) -> &TokenBreakdown {
        &self.accumulated_tokens
    }

    /// Number of turns recorded.
    pub fn total_turns(&self) -> u32 {
        self.total_turns
    }

    /// Per-turn cost history.
    pub fn turn_costs(&self) -> &[f64] {
        &self.turn_costs
    }

    /// Budget configuration.
    pub fn budget(&self) -> &BudgetConfig {
        &self.budget
    }

    /// Build a `CostEntry` from accumulated data for persistence.
    pub fn to_cost_entry(&self, session_id: &str, project: &str) -> CostEntry {
        CostEntry {
            session_id: session_id.to_string(),
            project: project.to_string(),
            timestamp: Utc::now(),
            cost_usd: self.accumulated_cost,
            tokens: self.accumulated_tokens.clone(),
            turns: self.total_turns,
            duration_ms: self.total_duration_ms,
            actor: self.actor.clone(),
            model: self.model.clone(),
        }
    }

    /// Human-readable cost string.
    pub fn cost_display(&self) -> String {
        crate::utils::typography::format_cost(self.accumulated_cost)
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
