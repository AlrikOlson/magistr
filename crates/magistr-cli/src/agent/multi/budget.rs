//! Resource budgeting — distribute and enforce token/cost limits across agents.
//!
//! Two levels of budget:
//! - `AgentBudget`: per-agent limits on tokens, cost, and turns.
//! - `PoolBudget`: aggregate limits across the entire agent pool.

use serde::{Deserialize, Serialize};

// ─── Agent Budget ──────────────────────────────────────────────────────────

/// Per-agent resource budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBudget {
    /// Maximum tokens this agent can consume (0 = unlimited).
    pub max_tokens: u64,
    /// Maximum cost in USD (0.0 = unlimited).
    pub max_cost_usd: f64,
    /// Maximum number of turns (0 = unlimited).
    pub max_turns: u32,
}

impl AgentBudget {
    /// Create an unlimited agent budget (no limits).
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            max_tokens: 0,
            max_cost_usd: 0.0,
            max_turns: 0,
        }
    }

    /// Check if any limit has been exceeded.
    #[must_use]
    pub fn is_exceeded(&self, tokens_used: u64, cost_usd: f64, turns: u32) -> bool {
        (self.max_tokens > 0 && tokens_used > self.max_tokens)
            || (self.max_cost_usd > 0.0 && cost_usd > self.max_cost_usd)
            || (self.max_turns > 0 && turns > self.max_turns)
    }

    /// Get the reason for budget exceedance, if any.
    #[must_use]
    pub fn exceeded_reason(&self, tokens_used: u64, cost_usd: f64, turns: u32) -> Option<String> {
        if self.max_tokens > 0 && tokens_used > self.max_tokens {
            Some(format!(
                "token limit exceeded: {tokens_used}/{} tokens",
                self.max_tokens
            ))
        } else if self.max_cost_usd > 0.0 && cost_usd > self.max_cost_usd {
            Some(format!(
                "cost limit exceeded: ${cost_usd:.4}/${:.4} USD",
                self.max_cost_usd
            ))
        } else if self.max_turns > 0 && turns > self.max_turns {
            Some(format!(
                "turn limit exceeded: {turns}/{} turns",
                self.max_turns
            ))
        } else {
            None
        }
    }
}

impl Default for AgentBudget {
    fn default() -> Self {
        Self::unlimited()
    }
}

// ─── Pool Budget ───────────────────────────────────────────────────────────

/// Aggregate resource budget across all agents in the pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolBudget {
    /// Maximum total tokens across all agents (0 = unlimited).
    pub max_total_tokens: u64,
    /// Maximum total cost in USD across all agents (0.0 = unlimited).
    pub max_total_cost_usd: f64,
    /// Maximum number of concurrent workers (0 = unlimited).
    pub max_concurrent_workers: u32,
    /// Per-agent default budget (applied to new workers).
    pub per_agent_default: AgentBudget,
}

impl PoolBudget {
    /// Create an unlimited pool budget.
    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            max_total_tokens: 0,
            max_total_cost_usd: 0.0,
            max_concurrent_workers: 0,
            per_agent_default: AgentBudget::unlimited(),
        }
    }

    /// Check if pool-level limits have been exceeded.
    #[must_use]
    pub fn is_exceeded(&self, total_cost_usd: f64, total_tokens: u64) -> bool {
        (self.max_total_tokens > 0 && total_tokens > self.max_total_tokens)
            || (self.max_total_cost_usd > 0.0 && total_cost_usd > self.max_total_cost_usd)
    }

    /// Get the reason for pool budget exceedance, if any.
    #[must_use]
    pub fn exceeded_reason(&self, total_cost_usd: f64, total_tokens: u64) -> Option<String> {
        if self.max_total_tokens > 0 && total_tokens > self.max_total_tokens {
            Some(format!(
                "pool token limit exceeded: {total_tokens}/{} tokens",
                self.max_total_tokens
            ))
        } else if self.max_total_cost_usd > 0.0 && total_cost_usd > self.max_total_cost_usd {
            Some(format!(
                "pool cost limit exceeded: ${total_cost_usd:.4}/${:.4} USD",
                self.max_total_cost_usd
            ))
        } else {
            None
        }
    }

    /// Whether the pool can accept another worker given the concurrency limit.
    #[must_use]
    pub fn can_add_worker(&self, current_active: u32) -> bool {
        self.max_concurrent_workers == 0 || current_active < self.max_concurrent_workers
    }
}

impl Default for PoolBudget {
    fn default() -> Self {
        Self::unlimited()
    }
}

// ─── Role-Weight Budget Distribution ──────────────────────────────────────

/// Default cost weight for a role. Higher weight = larger budget share.
///
/// Weights reflect typical resource consumption:
/// - Coder: heavy tool usage, most expensive → 1.0
/// - Tester: moderate tool usage → 0.6
/// - Reviewer: read-only, lightweight → 0.3
/// - Researcher: web search + reads, moderate → 0.4
/// - Documenter: mostly writes, lightweight → 0.3
#[must_use]
pub fn role_weight(role_id: &str) -> f64 {
    match role_id {
        "coder" => 1.0,
        "tester" => 0.6,
        "reviewer" => 0.3,
        "researcher" => 0.4,
        "documenter" => 0.3,
        _ => 0.5, // unknown roles get a moderate share
    }
}

/// Distribute a total cost budget across agents based on role weights.
///
/// Returns per-agent budgets proportional to their role weight.
/// If `total_cost_usd` is 0.0 (unlimited), returns unlimited budgets.
#[must_use]
pub fn distribute_budget(
    total_cost_usd: f64,
    total_tokens: u64,
    roles: &[&str],
) -> Vec<AgentBudget> {
    if roles.is_empty() {
        return Vec::new();
    }

    if total_cost_usd <= 0.0 && total_tokens == 0 {
        return roles.iter().map(|_| AgentBudget::unlimited()).collect();
    }

    let weights: Vec<f64> = roles.iter().map(|r| role_weight(r)).collect();
    let total_weight: f64 = weights.iter().sum();

    if total_weight <= 0.0 {
        return roles.iter().map(|_| AgentBudget::unlimited()).collect();
    }

    weights
        .iter()
        .map(|w| {
            let fraction = w / total_weight;
            AgentBudget {
                max_cost_usd: if total_cost_usd > 0.0 {
                    total_cost_usd * fraction
                } else {
                    0.0
                },
                max_tokens: if total_tokens > 0 {
                    (total_tokens as f64 * fraction) as u64
                } else {
                    0
                },
                max_turns: 0,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "budget.tests.rs"]
mod tests;
