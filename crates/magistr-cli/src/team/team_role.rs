//! The five-role model for team membership.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::capability::Capability;

/// A member's role in the team, ordered from most to least privileged.
///
/// Serialized in lowercase (`"owner"`, `"admin"`, …) in `.magistr/team.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TeamRole {
    /// Full control, including promoting and demoting Owners.
    Owner,
    /// Everything except transferring ownership.
    Admin,
    /// Builds: runs sessions and edits the roadmap.
    Developer,
    /// Read-only access.
    Viewer,
    /// Headless automation (CI pipelines): runs sessions and roadmap
    /// iterations, never answers permission prompts or edits the roster.
    Ci,
}

impl TeamRole {
    /// How privileged this role is; higher outranks lower.
    ///
    /// Declaration order cannot answer this — `Ci` is declared last but
    /// runs sessions and edits the roadmap, which puts it above
    /// `Viewer`. The ranking exists for one question: when several
    /// directory groups map a person to different roles, which wins
    /// ([`GroupRole::highest`]).
    ///
    /// [`GroupRole::highest`]: super::directory::GroupRole::highest
    #[must_use]
    pub fn privilege(self) -> u8 {
        match self {
            TeamRole::Owner => 5,
            TeamRole::Admin => 4,
            TeamRole::Developer => 3,
            TeamRole::Ci => 2,
            TeamRole::Viewer => 1,
        }
    }

    /// Whether this role grants `capability` per the fixed matrix.
    #[must_use]
    pub fn can(self, capability: Capability) -> bool {
        match self {
            TeamRole::Owner => true,
            TeamRole::Admin => !matches!(capability, Capability::TransferOwnership),
            TeamRole::Developer | TeamRole::Ci => matches!(
                capability,
                Capability::EditRoadmap | Capability::RunSessions | Capability::View
            ),
            TeamRole::Viewer => matches!(capability, Capability::View),
        }
    }
}

impl fmt::Display for TeamRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TeamRole::Owner => "owner",
            TeamRole::Admin => "admin",
            TeamRole::Developer => "developer",
            TeamRole::Viewer => "viewer",
            TeamRole::Ci => "ci",
        };
        f.write_str(s)
    }
}

impl FromStr for TeamRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "owner" => Ok(TeamRole::Owner),
            "admin" => Ok(TeamRole::Admin),
            "developer" | "dev" => Ok(TeamRole::Developer),
            "viewer" => Ok(TeamRole::Viewer),
            "ci" => Ok(TeamRole::Ci),
            other => Err(format!(
                "unknown role '{other}' — valid roles: owner, admin, developer, viewer, ci"
            )),
        }
    }
}

#[cfg(test)]
#[path = "team_role.tests.rs"]
mod tests;
