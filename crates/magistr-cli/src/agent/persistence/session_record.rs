//! A single session record stored in SQLite.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A single session record stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    /// Claude Agent SDK session ID.
    pub session_id: String,
    /// Project name or directory path.
    pub project: String,
    /// When the session started.
    pub started_at: DateTime<Utc>,
    /// When the session ended.
    pub ended_at: DateTime<Utc>,
    /// Total cost in USD.
    pub cost_usd: f64,
    /// Number of agent turns.
    pub turns: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Files changed during the session (relative paths).
    pub files_changed: Vec<String>,
    /// User-assigned tags for categorization.
    pub tags: Vec<String>,
    /// Brief summary of what was done (last assistant message or user-provided).
    pub summary: Option<String>,
    /// Model used for this session (e.g., "Claude Opus", "Claude Sonnet").
    #[serde(default)]
    pub model: String,
    /// Who started (or was handed) the session — the resolved team
    /// identity id (self-asserted attribution, not authentication).
    /// Absent in older rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

impl SessionRecord {
    /// Human-readable duration string.
    pub fn duration_display(&self) -> String {
        let secs = self.duration_ms / 1000;
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Human-readable cost string.
    pub fn cost_display(&self) -> String {
        crate::utils::typography::format_cost(self.cost_usd)
    }

    /// Number of files changed.
    pub fn file_count(&self) -> usize {
        self.files_changed.len()
    }
}

impl fmt::Display for SessionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | {} turns | {} files",
            &self.session_id[..8.min(self.session_id.len())],
            self.project,
            self.cost_display(),
            self.turns,
            self.file_count(),
        )
    }
}

#[cfg(test)]
#[path = "session_record.tests.rs"]
mod tests;
