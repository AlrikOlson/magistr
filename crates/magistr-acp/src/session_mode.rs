//! How the transport establishes its session.

/// New session, or resume an existing one.
#[derive(Debug, Clone)]
pub enum AcpSessionMode {
    /// `session/new`.
    New,
    /// `session/load` with the given provider id. A provider that does not
    /// advertise `loadSession` refuses this request before opening a conversation.
    Load {
        /// The session id to resume.
        session_id: String,
    },
}
