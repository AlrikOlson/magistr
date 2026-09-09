//! One recent session as the launcher's card lists it.

use super::session_outcome::SessionOutcome;

/// A recent session row, pre-formatted by the entry point — the cockpit
/// never opens the session database itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LauncherSession {
    /// The agent's session id, when the row can still be resumed;
    /// `None` for a record the entry point could not give one, which is
    /// the row the launcher refuses to launch.
    pub id: Option<String>,
    /// When the session started, formatted (`Aug 28 14:02`).
    pub when: String,
    /// How long it ran (`2m 30s`).
    pub duration: String,
    /// How the session ended; the view draws it in the icon set in force.
    pub outcome: SessionOutcome,
    /// The first line of the session's summary or prompt.
    pub preview: String,
}
