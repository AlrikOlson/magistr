//! Summary statistics for a set of cost entries.

use chrono::{DateTime, Utc};

use super::cost_entry::CostEntry;
use super::types::TokenBreakdown;

/// Summary statistics for a set of cost entries.
#[derive(Debug, Clone, PartialEq)]
pub struct CostSummary {
    /// Total cost when every entry has a reported amount.
    pub total_cost: Option<f64>,
    /// Total turns across all entries.
    pub total_turns: u32,
    /// Number of entries.
    pub entry_count: usize,
    /// Earliest timestamp (if any entries).
    pub earliest: Option<DateTime<Utc>>,
    /// Latest timestamp (if any entries).
    pub latest: Option<DateTime<Utc>>,
    /// Total tokens across all entries.
    pub total_tokens: TokenBreakdown,
}

impl CostSummary {
    /// Build a summary from a slice of cost entries.
    pub fn from_entries(entries: &[CostEntry]) -> Self {
        let mut total_cost = Some(0.0);
        let mut total_turns = 0u32;
        let mut earliest: Option<DateTime<Utc>> = None;
        let mut latest: Option<DateTime<Utc>> = None;
        let mut total_tokens = TokenBreakdown::default();

        for entry in entries {
            total_cost = total_cost
                .zip(entry.cost_usd)
                .map(|(total, cost)| total + cost);
            total_turns += entry.turns;
            total_tokens.accumulate(&entry.tokens);

            match earliest {
                None => earliest = Some(entry.timestamp),
                Some(e) if entry.timestamp < e => earliest = Some(entry.timestamp),
                _ => {}
            }
            match latest {
                None => latest = Some(entry.timestamp),
                Some(l) if entry.timestamp > l => latest = Some(entry.timestamp),
                _ => {}
            }
        }

        Self {
            total_cost,
            total_turns,
            entry_count: entries.len(),
            earliest,
            latest,
            total_tokens,
        }
    }

    /// Human-readable cost string.
    pub fn cost_display(&self) -> String {
        self.total_cost
            .map(crate::utils::typography::format_cost)
            .unwrap_or_else(|| "unknown".into())
    }
}
