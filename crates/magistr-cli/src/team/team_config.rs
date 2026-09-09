//! The team roster file — `.magistr/team.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MagistrError;

use magistr_engine::cost::{BudgetPolicy, BudgetRule};
use magistr_engine::policy::{GovernancePolicy, PolicyEngine};

use super::auth::IdentityProvider;
use super::directory::DirectoryConfig;
use super::identity::Identity;
use super::team_member::TeamMember;
use super::team_role::TeamRole;

/// The team configuration stored at `.magistr/team.json`.
///
/// Absence of the file is solo mode: no roster exists and the local user
/// resolves to `Owner`. Presence turns on role resolution for everyone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamConfig {
    /// Schema version, currently 1.
    #[serde(default = "version_default")]
    pub version: u32,
    /// Role for identities not on the roster.
    #[serde(default = "default_role_default")]
    pub default_role: TeamRole,
    /// The provider that must vouch for a member's identity.
    ///
    /// Absent (the default) uses local identity, which is
    /// self-asserted and nothing is refused. Present, and every gated
    /// command runs [`require_verified`] first.
    ///
    /// [`require_verified`]: super::auth::require_verified
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_provider: Option<IdentityProvider>,
    /// The hierarchical spending caps this team publishes.
    ///
    /// The rules live here, in the team artifact, rather than in a
    /// developer's own config — which is the whole point: resolution
    /// takes the minimum of these and the local budget, so a local
    /// `--budget` flag can tighten a cap but never raise one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub budgets: Vec<BudgetRule>,
    /// The guardrails this team enforces — approved tools, protected
    /// paths, required gates, approved models.
    ///
    /// Like `budgets`, these live in the team artifact rather than a
    /// developer's own config, and for the same reason: enforcement
    /// takes the *maximum* of the policy's effect and the local
    /// permission decision, so a local profile can tighten a guardrail
    /// but never loosen one.
    #[serde(default, skip_serializing_if = "GovernancePolicy::is_unset")]
    pub policy: GovernancePolicy,
    /// The corporate directory this team's membership comes from.
    ///
    /// Present, and membership is directory-driven: group→role mapping
    /// answers for identities the roster does not list, SCIM writes the
    /// roster instead of a human editing it, and a directory that does
    /// not answer refuses rather than falling back
    /// ([`resolve_role_with_directory`]). Absent — the default — and
    /// nothing in [`directory`](super::directory) is consulted at all.
    ///
    /// [`resolve_role_with_directory`]: super::directory::resolve_role_with_directory
    #[serde(default, skip_serializing_if = "DirectoryConfig::is_unset")]
    pub directory: DirectoryConfig,
    /// The roster.
    #[serde(default)]
    pub members: Vec<TeamMember>,
}

fn version_default() -> u32 {
    1
}

fn default_role_default() -> TeamRole {
    TeamRole::Viewer
}

impl TeamConfig {
    /// A fresh team with `identity` as the sole Owner.
    #[must_use]
    pub fn new_with_owner(identity: &Identity) -> Self {
        TeamConfig {
            version: version_default(),
            default_role: default_role_default(),
            identity_provider: None,
            budgets: Vec::new(),
            policy: GovernancePolicy::default(),
            directory: DirectoryConfig::default(),
            members: vec![TeamMember {
                id: identity.id().to_string(),
                name: Some(identity.name.clone()),
                role: TeamRole::Owner,
                active: true,
            }],
        }
    }

    /// The spending policy this roster publishes.
    #[must_use]
    pub fn policy(&self) -> BudgetPolicy {
        BudgetPolicy::new(self.budgets.clone())
    }

    /// The compiled guardrails this roster publishes.
    #[must_use]
    pub fn policy_engine(&self) -> PolicyEngine {
        self.policy.compile()
    }

    /// The governance policy a project directory publishes; an absent or
    /// unreadable roster is the empty policy — solo mode is ungoverned,
    /// never accidentally locked down.
    #[must_use]
    pub fn governance_for(dir: &Path) -> GovernancePolicy {
        Self::load(dir)
            .ok()
            .flatten()
            .map(|config| config.policy)
            .unwrap_or_default()
    }

    /// Where the roster lives for a project directory.
    #[must_use]
    pub fn path(dir: &Path) -> PathBuf {
        crate::paths::magistr_dir(dir).join("team.json")
    }

    /// Load the roster; `Ok(None)` when the file is absent (solo mode).
    pub fn load(dir: &Path) -> Result<Option<Self>, MagistrError> {
        let path = Self::path(dir);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(MagistrError::io(path, e)),
        };
        let config = serde_json::from_str(&text)
            .map_err(|e| MagistrError::parse("json", path.display().to_string(), e))?;
        Ok(Some(config))
    }

    /// Write the roster, creating `.magistr/` when needed.
    pub fn save(&self, dir: &Path) -> Result<(), MagistrError> {
        let magistr_dir = crate::paths::magistr_dir(dir);
        std::fs::create_dir_all(&magistr_dir).map_err(|e| MagistrError::io(&magistr_dir, e))?;
        let path = Self::path(dir);
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        std::fs::write(&path, text).map_err(|e| MagistrError::io(path, e))
    }

    /// Find the *provisioned* roster entry matching an identity
    /// (case-insensitive against the identity's email, then its name).
    ///
    /// A deactivated row is deliberately invisible here: every caller
    /// of this method is asking what somebody may do, and a
    /// deprovisioned member may do nothing. The row itself is still
    /// found by [`row_for`](Self::row_for), which is what SCIM
    /// reactivates and what
    /// [`resolve_role_with_directory`](super::directory::resolve_role_with_directory)
    /// refuses on.
    #[must_use]
    pub fn member_for(&self, identity: &Identity) -> Option<&TeamMember> {
        self.row_for(identity).filter(|m| m.active)
    }

    /// Find the roster entry matching an identity, provisioned or not.
    #[must_use]
    pub fn row_for(&self, identity: &Identity) -> Option<&TeamMember> {
        let matches_id = |candidate: &str| {
            identity
                .email
                .as_deref()
                .is_some_and(|email| email.eq_ignore_ascii_case(candidate))
                || identity.name.eq_ignore_ascii_case(candidate)
        };
        self.members.iter().find(|m| matches_id(&m.id))
    }

    /// The roster row for a member id, provisioned or not.
    #[must_use]
    pub fn row(&self, id: &str) -> Option<&TeamMember> {
        self.find(id).map(|index| &self.members[index])
    }

    /// The roster row for a member id, to be edited in place.
    pub fn row_mut(&mut self, id: &str) -> Option<&mut TeamMember> {
        self.find(id).map(|index| &mut self.members[index])
    }

    /// Add a member; refuses a duplicate id.
    pub fn add_member(&mut self, member: TeamMember) -> Result<(), MagistrError> {
        if self.find(&member.id).is_some() {
            return Err(MagistrError::Other(format!(
                "member '{}' is already on the roster",
                member.id
            )));
        }
        self.members.push(member);
        Ok(())
    }

    /// Remove a member; refuses an unknown id and the last Owner.
    pub fn remove_member(&mut self, id: &str) -> Result<(), MagistrError> {
        let index = self
            .find(id)
            .ok_or_else(|| MagistrError::Other(format!("no member '{id}' on the roster")))?;
        if self.members[index].role == TeamRole::Owner && self.owner_count() == 1 {
            return Err(MagistrError::Other(
                "cannot remove the last Owner — transfer ownership first".to_string(),
            ));
        }
        self.members.remove(index);
        Ok(())
    }

    /// Change a member's role; refuses an unknown id and demoting the
    /// last Owner.
    pub fn set_role(&mut self, id: &str, role: TeamRole) -> Result<(), MagistrError> {
        let index = self
            .find(id)
            .ok_or_else(|| MagistrError::Other(format!("no member '{id}' on the roster")))?;
        if self.members[index].role == TeamRole::Owner
            && role != TeamRole::Owner
            && self.owner_count() == 1
        {
            return Err(MagistrError::Other(
                "cannot demote the last Owner — promote another Owner first".to_string(),
            ));
        }
        self.members[index].role = role;
        Ok(())
    }

    /// The role a member id holds now, if the roster lists them.
    #[must_use]
    pub fn role_of(&self, id: &str) -> Option<TeamRole> {
        self.find(id).map(|index| self.members[index].role)
    }

    fn find(&self, id: &str) -> Option<usize> {
        self.members
            .iter()
            .position(|m| m.id.eq_ignore_ascii_case(id))
    }

    fn owner_count(&self) -> usize {
        self.members
            .iter()
            .filter(|m| m.role == TeamRole::Owner)
            .count()
    }
}

#[cfg(test)]
#[path = "team_config.tests.rs"]
mod tests;
