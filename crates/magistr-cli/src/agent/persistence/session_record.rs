//! Durable facts for one host-owned agent conversation.

use chrono::{DateTime, Utc};
use magistr_application::sessions::SessionPurpose;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::session_outcome::SessionOutcome;

/// One conversation checkpoint, identified by Magistr independently of its provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionRecord {
    pub session_id: String,
    /// Intent fixed when the conversation is created; controls startup authority.
    #[serde(
        serialize_with = "serialize_purpose",
        deserialize_with = "deserialize_purpose"
    )]
    pub purpose: SessionPurpose,
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
    /// The latest durable owner checkpoint.
    pub updated_at: DateTime<Utc>,
    /// Present only after the owner actually settles.
    pub ended_at: Option<DateTime<Utc>>,
    pub outcome: Option<SessionOutcome>,
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

pub(super) fn purpose_name(purpose: SessionPurpose) -> &'static str {
    match purpose {
        SessionPurpose::Conversation => "conversation",
        SessionPurpose::Roadmap => "roadmap",
    }
}

pub(super) fn purpose_from_name(value: &str) -> Result<SessionPurpose, String> {
    match value {
        "conversation" => Ok(SessionPurpose::Conversation),
        "roadmap" => Ok(SessionPurpose::Roadmap),
        _ => Err(format!("Unknown saved conversation purpose: {value}")),
    }
}

fn serialize_purpose<S: serde::Serializer>(
    purpose: &SessionPurpose,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(purpose_name(*purpose))
}

fn deserialize_purpose<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<SessionPurpose, D::Error> {
    let value = String::deserialize(deserializer)?;
    purpose_from_name(&value).map_err(serde::de::Error::custom)
}

#[cfg(test)]
#[path = "session_record.tests.rs"]
mod tests;
