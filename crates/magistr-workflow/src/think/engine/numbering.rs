//! Project-wide step-number bookkeeping.
//!
//! Step numbers are unique across every session that belongs to the
//! same project. The engine's `step_numbers` set covers only the
//! currently-loaded session; the methods here extend the check to
//! cover every *other* session in the project — important when
//! process_step needs to refuse a number that's already taken in a
//! sibling session.
//!

use crate::think::domain::SessionEntry;

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Iterate over every session entry that belongs to the same project
    /// as this server *and* is not the currently-active session. The active
    /// session's numbers live in `self.step_numbers` — combine the two
    /// when you need project-wide coverage.
    pub(crate) fn other_project_sessions(&self) -> impl Iterator<Item = &SessionEntry> {
        let active_sid = self.active_session.as_deref();
        let project_id = self.project_id.as_str();
        self.sessions.iter().filter_map(move |(sid, entry)| {
            if Some(sid.as_str()) == active_sid {
                return None;
            }
            let belongs = entry
                .history
                .metadata
                .as_ref()
                .and_then(|m| m.project_id.as_deref())
                .map_or_else(
                    || {
                        !project_id.is_empty()
                            && (sid == project_id
                                || sid.strip_prefix(project_id).is_some_and(|suffix| {
                                    suffix.starts_with(crate::think::config::PROJECT_SEP)
                                }))
                    },
                    |recorded| recorded == project_id,
                );
            if !belongs {
                return None;
            }
            Some(entry)
        })
    }

    /// True when `n` is recorded in the active session **or** any other
    /// session in this project. With an empty project_id (analysis-only
    /// servers, single-history mode) only the active session is consulted.
    pub(crate) fn step_number_taken_in_project(&self, n: u32) -> bool {
        if self.step_numbers.contains(&n) {
            return true;
        }
        if self.project_id.is_empty() {
            return false;
        }
        for entry in self.other_project_sessions() {
            if entry.history.steps.iter().any(|s| s.step_number == n) {
                return true;
            }
        }
        false
    }

    /// Highest step_number across the project (active session + every
    /// other session whose project_id matches). Returns 0 when nothing
    /// is recorded.
    pub(crate) fn max_step_number_in_project(&self) -> u32 {
        let active_max = self.step_numbers.iter().copied().max().unwrap_or(0);
        if self.project_id.is_empty() {
            return active_max;
        }
        let cross_max = self
            .other_project_sessions()
            .flat_map(|entry| entry.history.steps.iter().map(|s| s.step_number))
            .max()
            .unwrap_or(0);
        active_max.max(cross_max)
    }

    pub(crate) fn sorted_step_numbers(&self) -> String {
        let mut nums: Vec<u32> = self.step_numbers.iter().copied().collect();
        nums.sort_unstable();
        nums.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
