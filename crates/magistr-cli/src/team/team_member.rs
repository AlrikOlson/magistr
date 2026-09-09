//! One row of the team roster.

use serde::{Deserialize, Serialize};

use super::team_role::TeamRole;

/// A member of the team: a stable id (email or username) plus a role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMember {
    /// Matching key for identity resolution — an email or a username.
    /// Compared case-insensitively.
    pub id: String,
    /// Optional display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The member's role.
    pub role: TeamRole,
    /// Whether the member is provisioned.
    ///
    /// SCIM deprovisions with a soft delete — Okta and Entra both send
    /// `PATCH {op: replace, path: active, value: false}` rather than an
    /// HTTP DELETE — so a deactivated person stays on the roster as a
    /// row that grants nothing. Keeping the row is what makes
    /// reactivation a one-field change rather than a re-provision, and
    /// what stops a deactivated member from falling through to the
    /// roster's `default_role`: [`TeamConfig::member_for`] does not see
    /// them, and [`resolve_role_with_directory`] refuses them by name.
    ///
    /// Defaults to true and is not serialized when true, so no roster
    /// written before this field existed changes by a byte.
    ///
    /// [`TeamConfig::member_for`]: super::TeamConfig::member_for
    /// [`resolve_role_with_directory`]: super::directory::resolve_role_with_directory
    #[serde(default = "active_default", skip_serializing_if = "is_active")]
    pub active: bool,
}

fn active_default() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if takes a reference
fn is_active(active: &bool) -> bool {
    *active
}
