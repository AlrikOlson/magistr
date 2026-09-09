//! The `directory` block of `.magistr/team.json`.

use serde::{Deserialize, Serialize};

use super::group_role::GroupRole;
use super::ldap_config::LdapConfig;
use super::saml_config::SamlConfig;

/// What a team's directory is, and what membership of it buys.
///
/// SAML, LDAP and SCIM are three transports into one question — which
/// groups is this person in — so the group→role table lives here, once,
/// and each transport is a way of answering it. A team may configure
/// any of them, or none: `group_roles` alone is enough for the SCIM
/// path, which pushes membership at us rather than being asked.
///
/// The presence of this block is what turns on the fail-closed rule
/// ([`resolve_role_with_directory`]) — a team that has not configured a
/// directory is unaffected by every line of this module.
///
/// [`resolve_role_with_directory`]: super::resolve_role_with_directory
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryConfig {
    /// The group→role table every transport resolves through.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group_roles: Vec<GroupRole>,
    /// The SAML service-provider settings, when the team consumes
    /// assertions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saml: Option<SamlConfig>,
    /// The LDAP query, when the team looks groups up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ldap: Option<LdapConfig>,
}

impl DirectoryConfig {
    /// A directory that only maps groups to roles — the SCIM-only
    /// shape, and the one every test starts from.
    #[must_use]
    pub fn with_groups(group_roles: Vec<GroupRole>) -> Self {
        DirectoryConfig {
            group_roles,
            ..DirectoryConfig::default()
        }
    }

    /// Nothing configured at all — serialized away rather than written
    /// as an empty object.
    #[must_use]
    pub fn is_unset(&self) -> bool {
        *self == DirectoryConfig::default()
    }
}
