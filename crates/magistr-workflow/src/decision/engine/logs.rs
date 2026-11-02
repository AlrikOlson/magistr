//! Log validation and expiration of derived log views.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::decision::constants::LOG_CLEANUP_INTERVAL;
use crate::decision::domain::Note;

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Check all contextual references before changing the active log or
    /// expiring old logs. A rejected record must leave stored state alone.
    pub(crate) fn validate_log_note(&self, note: &mut Note) -> Result<(), String> {
        let target = note
            .log_id
            .as_deref()
            .filter(|_| self.config.features.enable_logs);
        let mut context;
        let source = if target.is_some() && target != self.active_log.as_deref() {
            let history = target
                .and_then(|id| self.logs.get(id))
                .map_or_else(Self::new_history, |entry| entry.history.clone());
            if !self.project_id.is_empty()
                && let Some(project) = history
                    .metadata
                    .as_ref()
                    .and_then(|meta| meta.project_id.as_deref())
                && project != self.project_id
            {
                return Err("The requested log belongs to another project".into());
            }
            let threads = Self::restored_threads(&history);
            context = Self::for_analysis(history, threads);
            context.config = self.config.clone();
            &context
        } else {
            self
        };
        source.validate_dependencies(note)?;
        source.resolve_thread(note)?;
        source.validate_revision(note)
    }

    pub(crate) fn log_timeout_ms(&self) -> u128 {
        u128::from(self.config.system.log_timeout) * 60 * 1000
    }

    pub(crate) fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    /// Drop log entries that haven't been accessed within the
    /// configured timeout window. Only runs every
    /// `LOG_CLEANUP_INTERVAL` process-note calls unless `force` is
    /// set; cheap no-op when logs are disabled.
    pub fn cleanup_expired_logs(&mut self, force: bool) {
        if !self.config.features.enable_logs {
            return;
        }
        self.notes_since_cleanup += 1;
        if !force && self.notes_since_cleanup < LOG_CLEANUP_INTERVAL {
            return;
        }
        self.notes_since_cleanup = 0;

        let now = Self::now_ms();
        let timeout = self.log_timeout_ms();
        let expired: Vec<String> = self
            .logs
            .iter()
            .filter_map(|(id, entry)| {
                if now.saturating_sub(entry.last_accessed) > timeout {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        for id in expired {
            self.logs.remove(&id);
            tracing::debug!("🗑️ Log {id} expired and removed");
        }
    }
}
