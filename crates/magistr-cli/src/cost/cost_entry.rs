//! A single cost entry representing one completed session.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::TokenBreakdown;

/// A single cost entry representing one completed session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostEntry {
    /// Logical Magistr conversation ID.
    pub session_id: String,
    /// Project name or directory.
    pub project: String,
    /// When the session ended.
    pub timestamp: DateTime<Utc>,
    /// Reported cumulative cost in USD; absent when the agent did not report it.
    pub cost_usd: Option<f64>,
    /// Token usage breakdown.
    #[serde(default)]
    pub tokens: TokenBreakdown,
    /// Number of agent turns.
    pub turns: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// The team identity this spend is charged to.
    ///
    /// Absent on every entry without attribution and on any solo run that
    /// never resolved an identity — reporting groups those under
    /// `unattributed` rather than dropping them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The model the session ran on, for per-model reporting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[cfg(test)]
#[path = "cost_entry.tests.rs"]
mod tests;
