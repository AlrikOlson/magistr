//! Who did the work, and what it cost — the review's identity block.

use super::copy::Copy;

/// The agent behind a session, as view 07's `THE AGENT` block names it:
/// the model, the charter mode it ran under, and what the run cost in
/// money, time and requests.
///
/// Seeded on the [`Rail`](super::Rail) beside the charter and the
/// budget, and for the same reason: nothing in the cockpit meters a
/// live session's model name or its request count, so this is the one
/// part of the review that is *asserted* rather than derived. Cost is
/// in cents so the rail's state stays `Eq` — the budget's own rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionAgent {
    model: String,
    mode: String,
    cost_cents: u64,
    minutes: u32,
    requests: u32,
}

impl SessionAgent {
    /// `model` running under charter `mode`, with nothing spent yet.
    #[must_use]
    pub fn new(model: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            mode: mode.into(),
            cost_cents: 0,
            minutes: 0,
            requests: 0,
        }
    }

    /// The same agent, having spent `cost_cents` over `minutes` in
    /// `requests` turns.
    #[must_use]
    pub const fn spent(mut self, cost_cents: u64, minutes: u32, requests: u32) -> Self {
        self.cost_cents = cost_cents;
        self.minutes = minutes;
        self.requests = requests;
        self
    }

    /// The first line — `sonnet · Draft mode`. A session bound under
    /// no mode word names the model alone rather than a dangling dot.
    #[must_use]
    pub fn identity(&self) -> String {
        if self.mode.is_empty() {
            self.model.clone()
        } else {
            format!("{} · {}", self.model, self.mode)
        }
    }

    /// The second line — `0.61 USD · 12 min · 2 requests`. A single
    /// request is not pluralised, because a block that says
    /// `1 requests` reads as a template rather than as a report.
    #[must_use]
    pub fn ledger(&self) -> String {
        let plural = if self.requests == 1 { "" } else { "s" };
        format!(
            "{} · {} min · {} request{plural}",
            Copy::usd(self.cost_cents),
            self.minutes,
            self.requests
        )
    }
}

#[cfg(test)]
#[path = "session_agent.tests.rs"]
mod tests;
