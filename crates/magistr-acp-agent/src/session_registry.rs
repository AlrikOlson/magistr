//! The open sessions, and the cancel-at-enqueue rule that makes the turn
//! queue and `session/cancel` compose.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use agent_client_protocol::schema::v1::SessionId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::AgentError;
use crate::queued_turn::QueuedTurn;
use crate::session_entry::SessionEntry;

/// Every session this agent has open, shared by the handlers and the workers.
///
/// A session is a queue and a cancellation. `session/prompt` enqueues a turn
/// carrying the cancellation that is live at that moment; `session/cancel`
/// cancels it and installs a fresh one. So the in-flight turn stops, every
/// turn queued before the cancel reaches its worker already cancelled and is
/// answered without touching the provider, and a prompt sent after the cancel
/// runs on the new cancellation — no state machine, no races.
#[derive(Clone, Default)]
pub(crate) struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<SessionId, SessionEntry>>>,
    minted: Arc<AtomicU64>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A fresh session id: a UUID v4, because a provider may hand it to the
    /// model runtime as the session's own id (`claude --session-id` takes
    /// nothing else) and resume it by the same id later.
    pub fn mint_id(&self) -> SessionId {
        self.minted.fetch_add(1, Ordering::SeqCst);
        SessionId::new(uuid::Uuid::new_v4().to_string())
    }

    /// Open a session on `model`, returning the queue its worker drains.
    pub fn open(
        &self,
        session_id: SessionId,
        model: Option<String>,
    ) -> Result<mpsc::UnboundedReceiver<QueuedTurn>, AgentError> {
        let mut sessions = self.lock();
        if sessions.contains_key(&session_id) {
            return Err(AgentError::SessionExists(session_id.to_string()));
        }
        let (queue, rx) = mpsc::unbounded_channel();
        sessions.insert(
            session_id,
            SessionEntry {
                queue,
                cancel: CancellationToken::new(),
                model,
            },
        );
        Ok(rx)
    }

    /// Admit a turn on its session: the queue to send it on and the
    /// cancellation it inherits — the one live now. Fails when the session is
    /// unknown.
    pub fn admit(
        &self,
        session_id: &SessionId,
    ) -> Result<(mpsc::UnboundedSender<QueuedTurn>, CancellationToken), AgentError> {
        let sessions = self.lock();
        let entry = sessions
            .get(session_id)
            .ok_or_else(|| AgentError::UnknownSession(session_id.to_string()))?;
        Ok((entry.queue.clone(), entry.cancel.clone()))
    }

    /// Cancel the session's live turn (and every turn queued before now) and
    /// arm a fresh cancellation for the turns that follow.
    pub fn cancel(&self, session_id: &SessionId) -> Result<(), AgentError> {
        let mut sessions = self.lock();
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentError::UnknownSession(session_id.to_string()))?;
        let cancelled = std::mem::replace(&mut entry.cancel, CancellationToken::new());
        cancelled.cancel();
        Ok(())
    }

    /// Switch the model the session's next turns run on.
    pub fn set_model(&self, session_id: &SessionId, model: String) -> Result<(), AgentError> {
        let mut sessions = self.lock();
        let entry = sessions
            .get_mut(session_id)
            .ok_or_else(|| AgentError::UnknownSession(session_id.to_string()))?;
        entry.model = Some(model);
        Ok(())
    }

    /// The model the session is on.
    pub fn model_of(&self, session_id: &SessionId) -> Result<Option<String>, AgentError> {
        let sessions = self.lock();
        sessions
            .get(session_id)
            .map(|entry| entry.model.clone())
            .ok_or_else(|| AgentError::UnknownSession(session_id.to_string()))
    }

    /// Whether the session is open.
    pub fn contains(&self, session_id: &SessionId) -> bool {
        self.lock().contains_key(session_id)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, SessionEntry>> {
        // A panic while holding the lock leaves the map intact; a poisoned
        // registry is still the registry.
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "session_registry.tests.rs"]
mod tests;
