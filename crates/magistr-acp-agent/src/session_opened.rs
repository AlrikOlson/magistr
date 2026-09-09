//! What a provider answers when a session has opened.

use agent_client_protocol::schema::v1::{SessionId, SessionUpdate};

/// A session the provider opened (`session/new`) or restored
/// (`session/load`): the id it is known by from here on, and the history to
/// replay to the client before the session is reported open.
///
/// The id is normally the one the [`SessionStart`](crate::SessionStart)
/// proposed. A provider whose runtime mints its own ids — Codex names every
/// thread itself — answers with that id instead, so the client resumes the
/// session later by the name the runtime knows it under. On `session/load`
/// the id must be the one the client named.
#[derive(Debug, Clone)]
pub struct SessionOpened {
    /// The session's id from here on.
    pub session_id: SessionId,
    /// The model the runtime opened or restored, when it selected one. It
    /// must be in the provider's model catalog. `None` uses the default.
    pub model: Option<String>,
    /// The history to replay; empty for a new session.
    pub replay: Vec<SessionUpdate>,
}

impl SessionOpened {
    /// A freshly opened session with nothing to replay.
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            model: None,
            replay: Vec::new(),
        }
    }

    /// Preserve the runtime's selected model for the picker and next turn.
    #[must_use]
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Attach the history to replay.
    #[must_use]
    pub fn replay(mut self, replay: Vec<SessionUpdate>) -> Self {
        self.replay = replay;
        self
    }
}
