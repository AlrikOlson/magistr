//! Configurable retention bounds for the permission audit log.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::audit_entry::AuditEntry;

/// How long audit entries live: an age bound in days, a count cap, or
/// both. The default is unlimited — nothing is ever pruned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRetention {
    /// Entries older than this many days are pruned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<u32>,
    /// At most this many entries are kept; the newest survive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_entries: Option<usize>,
}

impl AuditRetention {
    /// True when no bound is configured — pruning is a no-op.
    #[must_use]
    pub fn is_unlimited(&self) -> bool {
        self.days.is_none() && self.max_entries.is_none()
    }

    /// Apply both bounds to `entries` (append-ordered, oldest first) as
    /// of `now`: drop entries older than the day bound, then keep the
    /// last `max_entries`. Order is preserved.
    #[must_use]
    pub fn apply(&self, mut entries: Vec<AuditEntry>, now: DateTime<Utc>) -> Vec<AuditEntry> {
        if let Some(days) = self.days {
            let cutoff = now - chrono::Duration::days(i64::from(days));
            entries.retain(|e| e.timestamp >= cutoff);
        }
        if let Some(max) = self.max_entries
            && entries.len() > max
        {
            entries.drain(..entries.len() - max);
        }
        entries
    }
}

#[cfg(test)]
#[path = "audit_retention.tests.rs"]
mod tests;
