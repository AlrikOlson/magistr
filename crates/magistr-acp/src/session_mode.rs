//! How the transport establishes its session.

/// New session, or resume an existing one.
#[derive(Debug, Clone)]
pub enum AcpSessionMode {
    /// `session/new`.
    New,
    /// `session/load` with the given id. Falls back to `session/new` (with a
    /// warning event) when the agent does not advertise `loadSession`.
    Load {
        /// The session id to resume.
        session_id: String,
    },
}
