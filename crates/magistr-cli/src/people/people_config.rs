//! The team roster file — `.magistr/people.json`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MagistrError;

use magistr_engine::cost::{BudgetPolicy, BudgetRule};
use magistr_engine::policy::{GovernancePolicy, PolicyEngine};

use super::auth::IdentityProvider;
use super::directory::DirectoryConfig;
use super::identity::Identity;
use super::person::Person;
use super::person_role::PersonRole;

/// The team configuration stored at `.magistr/people.json`.
///
/// Absence of the file is solo mode: no roster exists and the local user
/// resolves to `Owner`. Presence turns on role resolution for everyone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeopleConfig {
    /// Schema version, currently 1.
    #[serde(default = "version_default")]
    pub version: u32,
    /// Role for identities not on the roster.
    #[serde(default = "default_role_default")]
    pub default_role: PersonRole,
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
    /// paths, required checks, approved models.
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
    pub members: Vec<Person>,
}

fn version_default() -> u32 {
    1
}

fn default_role_default() -> PersonRole {
    PersonRole::Viewer
}

impl PeopleConfig {
    /// A fresh team with `identity` as the sole Owner.
    #[must_use]
    pub fn new_with_owner(identity: &Identity) -> Self {
        PeopleConfig {
            version: version_default(),
            default_role: default_role_default(),
            identity_provider: None,
            budgets: Vec::new(),
            policy: GovernancePolicy::default(),
            directory: DirectoryConfig::default(),
            members: vec![Person {
                id: identity.id().to_string(),
                name: Some(identity.name.clone()),
                role: PersonRole::Owner,
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
        crate::paths::magistr_dir(dir).join("people.json")
    }

    /// Load the roster; `Ok(None)` when the file is absent (solo mode).
    ///
    /// A project that still keeps its roster at the pre-glossary
    /// `.magistr/team.json` is upgraded first, once, by
    /// [`upgrade_team_file`], and a roster whose policy still names its
    /// required checks "gates" is converted once by [`upgrade_policy_words`].
    pub fn load(dir: &Path) -> Result<Option<Self>, MagistrError> {
        upgrade_team_file(dir)?;
        upgrade_policy_words(dir)?;
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
    pub fn member_for(&self, identity: &Identity) -> Option<&Person> {
        self.row_for(identity).filter(|m| m.active)
    }

    /// Find the roster entry matching an identity, provisioned or not.
    #[must_use]
    pub fn row_for(&self, identity: &Identity) -> Option<&Person> {
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
    pub fn row(&self, id: &str) -> Option<&Person> {
        self.find(id).map(|index| &self.members[index])
    }

    /// The roster row for a member id, to be edited in place.
    pub fn row_mut(&mut self, id: &str) -> Option<&mut Person> {
        self.find(id).map(|index| &mut self.members[index])
    }

    /// Add a member; refuses a duplicate id.
    pub fn add_member(&mut self, member: Person) -> Result<(), MagistrError> {
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
        if self.members[index].role == PersonRole::Owner && self.owner_count() == 1 {
            return Err(MagistrError::Other(
                "cannot remove the last Owner — transfer ownership first".to_string(),
            ));
        }
        self.members.remove(index);
        Ok(())
    }

    /// Change a member's role; refuses an unknown id and demoting the
    /// last Owner.
    pub fn set_role(&mut self, id: &str, role: PersonRole) -> Result<(), MagistrError> {
        let index = self
            .find(id)
            .ok_or_else(|| MagistrError::Other(format!("no member '{id}' on the roster")))?;
        if self.members[index].role == PersonRole::Owner
            && role != PersonRole::Owner
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
    pub fn role_of(&self, id: &str) -> Option<PersonRole> {
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
            .filter(|m| m.role == PersonRole::Owner)
            .count()
    }
}

/// Move a pre-glossary `.magistr/team.json` to `.magistr/people.json`.
///
/// Runs only while `people.json` is absent, so it happens once and a
/// second call is a no-op. The new file is written to a temporary name
/// and renamed into place before the old one moves to
/// `team.json.backup`, so an interruption leaves either the untouched
/// original or a complete roster. Protected-path guards that named the
/// old file are pointed at the new one, so the roster stays guarded.
fn upgrade_team_file(dir: &Path) -> Result<(), MagistrError> {
    let magistr_dir = crate::paths::magistr_dir(dir);
    let legacy = magistr_dir.join("team.json");
    let target = PeopleConfig::path(dir);
    if target.exists() || !legacy.exists() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&legacy).map_err(|e| MagistrError::io(&legacy, e))?;
    let mut roster: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| MagistrError::parse("json", legacy.display().to_string(), e))?;
    if let Some(paths) = roster
        .pointer_mut("/policy/protected_paths")
        .and_then(serde_json::Value::as_array_mut)
    {
        for path in paths.iter_mut() {
            if let Some(guard) = path.as_str().and_then(|p| p.strip_suffix("team.json")) {
                *path = serde_json::Value::String(format!("{guard}people.json"));
            }
        }
    }
    let mut upgraded = serde_json::to_string_pretty(&roster)?;
    upgraded.push('\n');
    let staging = magistr_dir.join("people.json.upgrading");
    std::fs::write(&staging, upgraded).map_err(|e| MagistrError::io(&staging, e))?;
    std::fs::rename(&staging, &target).map_err(|e| MagistrError::io(&target, e))?;
    let backup = magistr_dir.join("team.json.backup");
    std::fs::rename(&legacy, &backup).map_err(|e| MagistrError::io(&backup, e))
}

/// Policy keys that named required checks "gates" before the check words.
const POLICY_CHECK_KEYS: [(&str, &str); 2] = [
    ("required_gates", "required_checks"),
    ("allow_gate_bypass", "allow_check_bypass"),
];

/// Convert a roster whose governance policy uses the gate words, once.
/// The policy ignores unknown keys, so an unconverted roster would silently
/// drop its required checks; the original is kept beside it as
/// `people.json.before-check-words`. Holding both spellings is refused.
fn upgrade_policy_words(dir: &Path) -> Result<(), MagistrError> {
    let path = PeopleConfig::path(dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(MagistrError::io(path, e)),
    };
    let mut roster: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| MagistrError::parse("json", path.display().to_string(), e))?;
    let Some(policy) = roster
        .get_mut("policy")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return Ok(());
    };
    let mut changed = false;
    for (old, new) in POLICY_CHECK_KEYS {
        let Some(value) = policy.remove(old) else {
            continue;
        };
        if policy.contains_key(new) {
            return Err(MagistrError::Config(format!(
                "{} holds both {old} and {new}; keep one and retry.",
                path.display()
            )));
        }
        policy.insert(new.to_owned(), value);
        changed = true;
    }
    if !changed {
        return Ok(());
    }
    let magistr_dir = crate::paths::magistr_dir(dir);
    let backup = magistr_dir.join("people.json.before-check-words");
    if !backup.exists() {
        std::fs::write(&backup, &text).map_err(|e| MagistrError::io(&backup, e))?;
    }
    let mut upgraded = serde_json::to_string_pretty(&roster)?;
    upgraded.push('\n');
    let staging = magistr_dir.join("people.json.upgrading");
    std::fs::write(&staging, upgraded).map_err(|e| MagistrError::io(&staging, e))?;
    std::fs::rename(&staging, &path).map_err(|e| MagistrError::io(&path, e))
}

#[cfg(test)]
#[path = "people_config.tests.rs"]
mod tests;
