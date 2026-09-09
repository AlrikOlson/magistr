use serde::{Deserialize, Serialize};

use super::cost_attribution::CostAttribution;
use super::token_type::TokenType;

/// One attributed observation of spend — the payload the cost channel
/// carries and the metrics exporters read.
///
/// [`CostEvent::CostUpdated`] answers "how much has this session spent";
/// a sample answers "who spent it, on which model, against whose cap",
/// which is the difference between one flat counter and a series a
/// dashboard can slice. It is deliberately a *turn-level* record with
/// the running total alongside: a histogram wants the delta, a gauge
/// wants the total, and deriving one from the other after the fact is
/// where double-counting comes from.
///
/// [`CostEvent::CostUpdated`]: super::cost_event::CostEvent::CostUpdated
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostSample {
    /// Who and what this spend is charged to.
    #[serde(default)]
    pub attribution: CostAttribution,
    /// The `gen_ai.operation.name` this spend was incurred under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// Cost of the turn this sample describes.
    pub turn_cost_usd: f64,
    /// Session cost including this turn.
    pub total_cost_usd: f64,
    /// Tokens the model consumed on this turn.
    #[serde(default)]
    pub input_tokens: u64,
    /// Tokens the model produced on this turn.
    #[serde(default)]
    pub output_tokens: u64,
    /// Wall-clock duration of the turn, in seconds.
    #[serde(default)]
    pub duration_s: f64,
}

impl CostSample {
    /// The `gen_ai.operation.name` used when none is given.
    ///
    /// Magistr drives a coding agent over ACP rather than issuing a bare
    /// completion, so `invoke_agent` is the convention's value for what
    /// is actually happening — not `chat`.
    pub const DEFAULT_OPERATION: &'static str = "invoke_agent";

    /// A sample for one turn's spend and token usage.
    #[must_use]
    pub fn new(turn_cost_usd: f64, input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            turn_cost_usd,
            total_cost_usd: turn_cost_usd,
            input_tokens,
            output_tokens,
            ..Self::default()
        }
    }

    /// Charge this sample to an attribution (self-consuming builder).
    #[must_use]
    pub fn with_attribution(mut self, attribution: CostAttribution) -> Self {
        self.attribution = attribution;
        self
    }

    /// Set the session total this turn rolls into (self-consuming builder).
    #[must_use]
    pub fn with_total(mut self, total_cost_usd: f64) -> Self {
        self.total_cost_usd = total_cost_usd;
        self
    }

    /// Name the operation (self-consuming builder).
    #[must_use]
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Record how long the turn took (self-consuming builder).
    #[must_use]
    pub fn with_duration_s(mut self, duration_s: f64) -> Self {
        self.duration_s = duration_s;
        self
    }

    /// The `gen_ai.operation.name` label — never empty.
    #[must_use]
    pub fn operation_label(&self) -> &str {
        match self.operation.as_deref().map(str::trim) {
            Some(op) if !op.is_empty() => op,
            _ => Self::DEFAULT_OPERATION,
        }
    }

    /// Tokens of one type on this turn.
    #[must_use]
    pub const fn tokens(&self, kind: TokenType) -> u64 {
        match kind {
            TokenType::Input => self.input_tokens,
            TokenType::Output => self.output_tokens,
        }
    }

    /// Whether this sample carries anything worth exporting.
    ///
    /// A turn that cost nothing and moved no tokens is a heartbeat, not
    /// an observation; recording it would add a series with no data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.turn_cost_usd <= 0.0 && self.input_tokens == 0 && self.output_tokens == 0
    }
}

#[cfg(test)]
#[path = "cost_sample.tests.rs"]
mod tests;
