//! What someone is trying to do with a session.

/// An access-controlled operation on a recorded or running session.
///
/// Paired with [`super::session_access`]: the capability matrix decides
/// which roles may do what, and whether owning the session matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionAction {
    /// Read a session's record, history, and summary.
    View,
    /// Attach to a running session and drive it.
    Attach,
    /// Take over a session owned by someone else (handoff).
    TakeOver,
}
