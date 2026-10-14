//! Durable facts for one host-owned agent conversation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::session_outcome::SessionOutcome;

/// One settled conversation, identified by Magistr independently of its provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub session_id: String,
    /// A name assigned through the owning conversation.
    pub name: Option<String>,
    /// Canonical project authority directory shared by its working copies.
    pub project: String,
    /// Canonical working directory in which this conversation ran.
    pub workspace: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    /// The negotiated provider supports loading this conversation.
    pub resume_supported: bool,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub outcome: SessionOutcome,
    /// Reported cumulative USD cost; absent when the provider did not price it.
    pub cost_usd: Option<f64>,
    /// Terminal responses observed by the owner; absent when not recorded.
    pub turns: Option<u32>,
    pub duration_ms: u64,
    pub files_changed: Vec<String>,
    pub tags: Vec<String>,
    /// Bounded text from the latest assistant response.
    pub summary: Option<String>,
    pub model: Option<String>,
    /// Resolved local attribution, or the person this session was handed to.
    pub owner: Option<String>,
}

impl SessionRecord {
    #[must_use]
    pub fn can_resume(&self) -> bool {
        self.resume_supported && self.provider_session_id.is_some()
    }

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

    pub fn cost_display(&self) -> String {
        self.cost_usd
            .map(crate::utils::typography::format_cost)
            .unwrap_or_else(|| "unknown".into())
    }

    pub fn turns_display(&self) -> String {
        self.turns
            .map(|turns| turns.to_string())
            .unwrap_or_else(|| "unknown".into())
    }

    pub fn file_count(&self) -> usize {
        self.files_changed.len()
    }
}

impl fmt::Display for SessionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id: String = self.session_id.chars().take(8).collect();
        write!(
            f,
            "{id} | {} | {} | {} turns | {} files",
            self.workspace,
            self.cost_display(),
            self.turns_display(),
            self.file_count(),
        )
    }
}

#[cfg(test)]
#[path = "session_record.tests.rs"]
mod tests;
