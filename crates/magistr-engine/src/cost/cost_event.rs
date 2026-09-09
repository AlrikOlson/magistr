use serde::{Deserialize, Serialize};

use super::cost_sample::CostSample;

/// Events emitted by the cost governor.
///
/// These domain events notify consumers about cost state changes and
/// budget policy enforcement actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CostEvent {
    /// Cost was updated after recording a turn.
    CostUpdated {
        /// Accumulated total cost in USD.
        total_cost_usd: f64,
        /// Cost for this turn.
        turn_cost_usd: f64,
        /// Current turn count.
        turns: u32,
    },

    /// Budget warning threshold has been crossed.
    BudgetWarning {
        /// Amount remaining before hard limit.
        remaining: f64,
        /// The warning threshold that was crossed.
        threshold: f64,
        /// Current total cost.
        total_cost_usd: f64,
    },

    /// Budget hard limit has been exceeded — session should stop.
    BudgetExceeded {
        /// Amount over the limit.
        overage: f64,
        /// The configured limit.
        limit: f64,
        /// Current total cost.
        total_cost_usd: f64,
    },

    /// One turn's spend, attributed — the payload the cost channel
    /// carries to metrics and OTLP exporters.
    ///
    /// Emitted alongside `CostUpdated` rather than replacing it: the
    /// existing variant is the session's running total and has
    /// consumers that only want that, while a sample is the labelled
    /// delta an exporter needs.
    CostSampled {
        /// The attributed observation.
        sample: CostSample,
    },

    /// Budget limit was extended (raised) during the session.
    BudgetExtended {
        /// Previous limit.
        old_limit: f64,
        /// New limit.
        new_limit: f64,
    },
}
