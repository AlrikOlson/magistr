//! What one finished agent session spent.

use super::types::TokenBreakdown;

/// The measurements a finished session hands the ledger.
///
/// Deliberately not `CostEntry`: an entry also carries the identity and
/// the project, which the caller at the end of a session does not know
/// and should not have to look up. This is the half the agent reports;
/// [`record_session`] resolves the other half.
///
/// [`record_session`]: super::record_session::record_session
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionSpend {
    /// The logical Magistr conversation ID.
    pub session_id: String,
    /// Total cost the agent reported, in USD.
    pub cost_usd: Option<f64>,
    /// Token usage, as far as the agent reported it.
    pub tokens: TokenBreakdown,
    /// Turns the session ran.
    pub turns: u32,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// The model the session ran on, when it is known.
    pub model: Option<String>,
}

impl SessionSpend {
    /// A record of a session that cost `cost_usd`.
    #[must_use]
    pub fn new(session_id: impl Into<String>, cost_usd: Option<f64>) -> Self {
        Self {
            session_id: session_id.into(),
            cost_usd,
            ..Self::default()
        }
    }

    /// Attach the token breakdown (self-consuming builder).
    #[must_use]
    pub fn with_tokens(mut self, tokens: TokenBreakdown) -> Self {
        self.tokens = tokens;
        self
    }

    /// Attach turn count and duration (self-consuming builder).
    #[must_use]
    pub fn with_run(mut self, turns: u32, duration_ms: u64) -> Self {
        self.turns = turns;
        self.duration_ms = duration_ms;
        self
    }

    /// Name the model (self-consuming builder).
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Read the token breakdown out of an agent's `usage` object.
    ///
    /// Claude's usage reports cache reads and cache writes separately
    /// from input; they are kept separate here too, because collapsing
    /// them would make a cached run look identical to an uncached one
    /// in the very report meant to tell them apart.
    #[must_use]
    pub fn tokens_from_usage(usage: &serde_json::Value) -> TokenBreakdown {
        let read = |key: &str| {
            usage
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        };
        TokenBreakdown {
            input_tokens: read("input_tokens"),
            output_tokens: read("output_tokens"),
            cache_read_tokens: read("cache_read_input_tokens"),
            cache_write_tokens: read("cache_creation_input_tokens"),
        }
    }

    /// Whether there is no session identity or reported measurement.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.session_id.is_empty()
            && self.cost_usd.is_none()
            && self.tokens.total() == 0
            && self.turns == 0
            && self.duration_ms == 0
    }
}

#[cfg(test)]
#[path = "session_spend.tests.rs"]
mod tests;
