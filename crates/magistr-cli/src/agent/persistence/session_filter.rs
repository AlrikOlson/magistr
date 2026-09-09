//! Filter criteria for querying sessions.

use chrono::{DateTime, Utc};

/// Filter criteria for querying sessions.
#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    /// Filter by project name (substring match).
    pub project: Option<String>,
    /// Only sessions after this timestamp.
    pub since: Option<DateTime<Utc>>,
    /// Only sessions with this tag.
    pub tag: Option<String>,
    /// Filter by owner identity (substring match).
    pub owner: Option<String>,
    /// Maximum number of results.
    pub limit: usize,
}

impl SessionFilter {
    /// Create a filter with default limit of 20.
    pub fn new() -> Self {
        Self {
            limit: 20,
            ..Default::default()
        }
    }

    /// Set project filter.
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Set since filter.
    pub fn with_since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Set tag filter.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Set owner filter.
    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }

    /// Set result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

#[cfg(test)]
#[path = "session_filter.tests.rs"]
mod tests;
