//! Session validation and expiration of derived session views.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::think::constants::SESSION_CLEANUP_INTERVAL;
use crate::think::domain::ThinkStep;

use super::core::ReasoningServer;

impl ReasoningServer {
    /// Check all contextual references before changing the active session or
    /// expiring old sessions. A rejected record must leave stored state alone.
    pub(crate) fn validate_session_step(&self, step: &mut ThinkStep) -> Result<(), String> {
        let target = step
            .session_id
            .as_deref()
            .filter(|_| self.config.features.enable_sessions);
        let mut context;
        let source = if target.is_some() && target != self.active_session.as_deref() {
            let history = target
                .and_then(|id| self.sessions.get(id))
                .map_or_else(Self::new_history, |entry| entry.history.clone());
            if !self.project_id.is_empty()
                && let Some(project) = history
                    .metadata
                    .as_ref()
                    .and_then(|meta| meta.project_id.as_deref())
                && project != self.project_id
            {
                return Err("The requested session belongs to another project".into());
            }
            let branches = Self::restored_branches(&history);
            context = Self::for_analysis(history, branches);
            context.config = self.config.clone();
            &context
        } else {
            self
        };
        source.validate_dependencies(step)?;
        source.resolve_branch(step)?;
        source.validate_revision(step)
    }

    pub(crate) fn session_timeout_ms(&self) -> u128 {
        u128::from(self.config.system.session_timeout) * 60 * 1000
    }

    pub(crate) fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    /// Drop session entries that haven't been accessed within the
    /// configured timeout window. Only runs every
    /// `SESSION_CLEANUP_INTERVAL` process-step calls unless `force` is
    /// set; cheap no-op when sessions are disabled.
    pub fn cleanup_expired_sessions(&mut self, force: bool) {
        if !self.config.features.enable_sessions {
            return;
        }
        self.steps_since_cleanup += 1;
        if !force && self.steps_since_cleanup < SESSION_CLEANUP_INTERVAL {
            return;
        }
        self.steps_since_cleanup = 0;

        let now = Self::now_ms();
        let timeout = self.session_timeout_ms();
        let expired: Vec<String> = self
            .sessions
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
            self.sessions.remove(&id);
            tracing::debug!("🗑️ Session {id} expired and removed");
        }
    }
}
