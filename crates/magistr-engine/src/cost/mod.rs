//! Cost governance — domain service for budget enforcement and cost tracking.
//!
//! `CostGovernor` owns the pure domain logic for spending limits, warning
//! thresholds, and budget policy enforcement. It is backend-agnostic:
//! no persistence, no TUI, no IO.

pub mod budget_config;
pub mod budget_decision;
pub mod budget_period;
pub mod budget_policy;
pub mod budget_rule;
pub mod budget_scope;
pub mod budget_status;
pub mod cost_attribution;
pub mod cost_event;
pub mod cost_governor;
pub mod cost_sample;
pub mod resolved_budget;
pub mod scope_headroom;
pub mod scope_spend;
pub mod token_type;

pub use budget_config::BudgetConfig;
pub use budget_decision::BudgetDecision;
pub use budget_period::BudgetPeriod;
pub use budget_policy::BudgetPolicy;
pub use budget_rule::BudgetRule;
pub use budget_scope::BudgetScope;
pub use budget_status::BudgetStatus;
pub use cost_attribution::CostAttribution;
pub use cost_event::CostEvent;
pub use cost_governor::CostGovernor;
pub use cost_sample::CostSample;
pub use resolved_budget::ResolvedBudget;
pub use scope_headroom::ScopeHeadroom;
pub use scope_spend::ScopeSpend;
pub use token_type::TokenType;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
