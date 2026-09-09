//! A single memory entry — one bullet point of knowledge.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::memory_category::MemoryCategory;

/// A single memory entry — one bullet point of knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// The category this entry belongs to.
    pub category: MemoryCategory,
    /// Human-readable text describing the observation.
    pub text: String,
    /// When this entry was added (if known).
    pub added_at: Option<DateTime<Utc>>,
}

impl MemoryEntry {
    /// Create a new entry without a timestamp.
    pub fn new(category: MemoryCategory, text: impl Into<String>) -> Self {
        Self {
            category,
            text: text.into(),
            added_at: None,
        }
    }

    /// Create a new entry with the current timestamp.
    pub fn now(category: MemoryCategory, text: impl Into<String>) -> Self {
        Self {
            category,
            text: text.into(),
            added_at: Some(Utc::now()),
        }
    }
}

#[cfg(test)]
#[path = "memory_entry.tests.rs"]
mod tests;
