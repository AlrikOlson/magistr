//! Role-based access decisions for roster edits.

use super::capability::Capability;
use super::roster_change::RosterChange;
use super::team_role::TeamRole;

/// Whether `role` may apply `change` to the roster.
///
/// Two rules, defined by the capability matrix
/// and nothing enforced until now: editing the roster at all needs
/// [`Capability::ManageTeam`], and any change that grants Owner or
/// alters an existing Owner additionally needs
/// [`Capability::TransferOwnership`] — which only an Owner holds.
///
/// The second rule is what stops the classic role-editing escalation:
/// an Admin has every other capability, so without it `magistr team
/// set-role me owner` would be a one-command promotion to the one role
/// they are not supposed to reach.
///
/// This is authorization, not integrity: [`super::TeamConfig`] still
/// refuses a duplicate id, an unknown id, and losing the last Owner.
#[must_use]
pub fn roster_access(role: TeamRole, change: RosterChange) -> bool {
    role.can(Capability::ManageTeam)
        && (!change.touches_owner() || role.can(Capability::TransferOwnership))
}

#[cfg(test)]
#[path = "roster_access.tests.rs"]
mod tests;
