//! Aggregate summary across multiple sessions.

use chrono::{DateTime, Utc};

use super::session_record::SessionRecord;

/// Aggregate summary across multiple sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    /// Total number of sessions.
    pub count: usize,
    /// Total cost across all sessions.
    pub total_cost: f64,
    /// Total turns across all sessions.
    pub total_turns: u32,
    /// Total files changed (may include duplicates across sessions).
    pub total_files: usize,
    /// Earliest session start.
    pub earliest: Option<DateTime<Utc>>,
    /// Latest session end.
    pub latest: Option<DateTime<Utc>>,
}

impl SessionSummary {
    /// Build a summary from a slice of session records.
    pub fn from_records(records: &[SessionRecord]) -> Self {
        let mut summary = Self {
            count: records.len(),
            total_cost: 0.0,
            total_turns: 0,
            total_files: 0,
            earliest: None,
            latest: None,
        };

        for r in records {
            summary.total_cost += r.cost_usd;
            summary.total_turns += r.turns;
            summary.total_files += r.files_changed.len();

            match summary.earliest {
                None => summary.earliest = Some(r.started_at),
                Some(e) if r.started_at < e => summary.earliest = Some(r.started_at),
                _ => {}
            }
            match summary.latest {
                None => summary.latest = Some(r.ended_at),
                Some(l) if r.ended_at > l => summary.latest = Some(r.ended_at),
                _ => {}
            }
        }

        summary
    }
}

#[cfg(test)]
#[path = "session_summary.tests.rs"]
mod tests;
