//! Role-based access decisions for sessions.

use super::capability::Capability;
use super::session_action::SessionAction;
use super::team_role::TeamRole;

/// Whether `role` may perform `action` on a session, given whether the
/// caller owns it.
///
/// The access matrix: every role that can view may view any session;
/// attaching requires the run-sessions capability and is limited to your
/// own sessions unless you can manage the team (Admin+); taking a session
/// over — the handoff — is the session owner's right, or Admin+'s.
#[must_use]
pub fn session_access(role: TeamRole, action: SessionAction, owns_session: bool) -> bool {
    match action {
        SessionAction::View => role.can(Capability::View),
        SessionAction::Attach => {
            role.can(Capability::RunSessions) && (owns_session || role.can(Capability::ManageTeam))
        }
        SessionAction::TakeOver => owns_session || role.can(Capability::ManageTeam),
    }
}

#[cfg(test)]
#[path = "session_access.tests.rs"]
mod tests;
