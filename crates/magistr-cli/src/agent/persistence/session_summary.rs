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
    /// Sessions that did not report a price.
    pub unknown_cost_sessions: usize,
    pub unknown_turn_sessions: usize,
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
            unknown_cost_sessions: 0,
            unknown_turn_sessions: 0,
            total_turns: 0,
            total_files: 0,
            earliest: None,
            latest: None,
        };

        for r in records {
            match r.cost_usd {
                Some(cost) => summary.total_cost += cost,
                None => summary.unknown_cost_sessions += 1,
            }
            match r.turns {
                Some(turns) => summary.total_turns += turns,
                None => summary.unknown_turn_sessions += 1,
            }
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
