//! The five-role model for team membership.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::capability::Capability;

/// A member's role in the team, ordered from most to least privileged.
///
/// Serialized in lowercase (`"owner"`, `"admin"`, …) in `.magistr/people.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonRole {
    /// Full control, including promoting and demoting Owners.
    Owner,
    /// Everything except transferring ownership.
    Admin,
    /// Builds: runs sessions and edits the plan.
    Developer,
    /// Read-only access.
    Viewer,
    /// Headless automation (CI pipelines): runs sessions and plan
    /// iterations, never answers permission prompts or edits the roster.
    Ci,
}

impl PersonRole {
    /// How privileged this role is; higher outranks lower.
    ///
    /// Declaration order cannot answer this — `Ci` is declared last but
    /// runs sessions and edits the plan, which puts it above
    /// `Viewer`. The ranking exists for one question: when several
    /// directory groups map a person to different roles, which wins
    /// ([`GroupRole::highest`]).
    ///
    /// [`GroupRole::highest`]: super::directory::GroupRole::highest
    #[must_use]
    pub fn privilege(self) -> u8 {
        match self {
            PersonRole::Owner => 5,
            PersonRole::Admin => 4,
            PersonRole::Developer => 3,
            PersonRole::Ci => 2,
            PersonRole::Viewer => 1,
        }
    }

    /// Whether this role grants `capability` per the fixed matrix.
    #[must_use]
    pub fn can(self, capability: Capability) -> bool {
        match self {
            PersonRole::Owner => true,
            PersonRole::Admin => !matches!(capability, Capability::TransferOwnership),
            PersonRole::Developer | PersonRole::Ci => matches!(
                capability,
                Capability::EditPlan | Capability::RunSessions | Capability::View
            ),
            PersonRole::Viewer => matches!(capability, Capability::View),
        }
    }
}

impl fmt::Display for PersonRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PersonRole::Owner => "owner",
            PersonRole::Admin => "admin",
            PersonRole::Developer => "developer",
            PersonRole::Viewer => "viewer",
            PersonRole::Ci => "ci",
        };
        f.write_str(s)
    }
}

impl FromStr for PersonRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "owner" => Ok(PersonRole::Owner),
            "admin" => Ok(PersonRole::Admin),
            "developer" | "dev" => Ok(PersonRole::Developer),
            "viewer" => Ok(PersonRole::Viewer),
            "ci" => Ok(PersonRole::Ci),
            other => Err(format!(
                "unknown role '{other}' — valid roles: owner, admin, developer, viewer, ci"
            )),
        }
    }
}

#[cfg(test)]
#[path = "person_role.tests.rs"]
mod tests;
