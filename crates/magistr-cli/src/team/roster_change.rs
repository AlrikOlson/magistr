//! A proposed change to the team roster.

use super::team_role::TeamRole;

/// One mutation of `.magistr/team.json`, described by the roles it
/// touches rather than by the member ids.
///
/// Paired with [`super::roster_access`], which decides whether the
/// acting role may ask for it. What matters to the decision is never
/// *who* is named but *which privilege level* the change creates or
/// disturbs: granting Owner, or altering somebody who already is one,
/// is ownership transfer whatever the member id says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterChange {
    /// Put a new member on the roster with this role.
    Add(TeamRole),
    /// Take a member off the roster; the role is the one they hold now.
    Remove(TeamRole),
    /// Move a member from one role to another.
    SetRole {
        /// The role the member holds now.
        from: TeamRole,
        /// The role they would hold after the change.
        to: TeamRole,
    },
}

impl RosterChange {
    /// Whether this change creates or disturbs an Owner — the test for
    /// [`super::Capability::TransferOwnership`].
    #[must_use]
    pub fn touches_owner(self) -> bool {
        match self {
            RosterChange::Add(role) | RosterChange::Remove(role) => role == TeamRole::Owner,
            RosterChange::SetRole { from, to } => from == TeamRole::Owner || to == TeamRole::Owner,
        }
    }
}
