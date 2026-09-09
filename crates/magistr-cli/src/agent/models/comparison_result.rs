//! Side-by-side comparison result from two models.

use super::model_response::ModelResponse;

/// Side-by-side comparison result from two models.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonResult {
    /// The original prompt.
    pub prompt: String,
    /// Response from model A.
    pub response_a: ModelResponse,
    /// Response from model B.
    pub response_b: ModelResponse,
}

impl ComparisonResult {
    /// Create a comparison result from two model responses.
    pub fn new(
        prompt: impl Into<String>,
        response_a: ModelResponse,
        response_b: ModelResponse,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            response_a,
            response_b,
        }
    }

    /// Cost difference: positive means model A is more expensive.
    pub fn cost_difference(&self) -> f64 {
        self.response_a.cost_usd - self.response_b.cost_usd
    }

    /// Which model was cheaper.
    pub fn cheaper_model(&self) -> &str {
        if self.response_a.cost_usd <= self.response_b.cost_usd {
            &self.response_a.display_name
        } else {
            &self.response_b.display_name
        }
    }

    /// Which model was faster.
    pub fn faster_model(&self) -> &str {
        if self.response_a.duration_ms <= self.response_b.duration_ms {
            &self.response_a.display_name
        } else {
            &self.response_b.display_name
        }
    }

    /// Cost savings percentage: how much cheaper is the cheaper model.
    pub fn cost_savings_percent(&self) -> f64 {
        let max_cost = self.response_a.cost_usd.max(self.response_b.cost_usd);
        if max_cost == 0.0 {
            return 0.0;
        }
        let min_cost = self.response_a.cost_usd.min(self.response_b.cost_usd);
        ((max_cost - min_cost) / max_cost) * 100.0
    }

    /// Speed difference percentage: how much faster is the faster model.
    pub fn speed_difference_percent(&self) -> f64 {
        let max_dur = self.response_a.duration_ms.max(self.response_b.duration_ms);
        if max_dur == 0 {
            return 0.0;
        }
        let min_dur = self.response_a.duration_ms.min(self.response_b.duration_ms);
        ((max_dur - min_dur) as f64 / max_dur as f64) * 100.0
    }
}
