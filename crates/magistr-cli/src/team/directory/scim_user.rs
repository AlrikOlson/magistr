//! A SCIM 2.0 User resource.

use serde::{Deserialize, Serialize};

use super::ldap_config::LdapConfig;
use super::scim_name::ScimName;
use super::scim_value::ScimValue;

/// The SCIM `User` resource, cut down to what a roster row records.
///
/// The unmodelled attributes of RFC 7643 §4.1 — `title`, `locale`,
/// `addresses`, `entitlements` and the rest — are simply not read.
/// That is not a silent drop: a roster row is an id, a name, a role and
/// whether the person is provisioned, so an attribute the roster cannot
/// store is an attribute whose change is genuinely a no-op here, and
/// [`apply_scim`](super::apply_scim) says which paths it ignored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimUser {
    /// The schema URIs the provider declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<String>,
    /// The service-provider id. magistr assigns the `userName`, so a
    /// roster needs no second identifier to keep in step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The provider's own id for this user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// The login name — the roster's member id.
    pub user_name: String,
    /// The display name, when the provider sends one directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// The structured name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<ScimName>,
    /// The user's email addresses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<ScimValue>,
    /// The groups the user belongs to, when the provider sends them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ScimValue>,
    /// Whether the user is provisioned. Absent means provisioned.
    #[serde(default = "active_default")]
    pub active: bool,
}

fn active_default() -> bool {
    true
}

impl ScimUser {
    /// A minimal user — the shape a `POST /Users` needs at least.
    #[must_use]
    pub fn new(user_name: impl Into<String>) -> Self {
        ScimUser {
            user_name: user_name.into(),
            active: true,
            ..ScimUser::default()
        }
    }

    /// The roster member id: the `userName`.
    ///
    /// Deliberately not a magistr-minted id. A service provider that
    /// assigns its own identifiers has to keep them in step with the
    /// IdP's; making the `userName` the id means there is only ever one
    /// key, and it is the one an administrator can read in both
    /// systems. For Okta and Entra it is the person's email, which is
    /// also what [`Identity::id`] resolves to.
    ///
    /// [`Identity::id`]: crate::team::Identity::id
    #[must_use]
    pub fn member_id(&self) -> &str {
        self.user_name.trim()
    }

    /// The display name to record, if the provider gave one.
    #[must_use]
    pub fn display(&self) -> Option<String> {
        self.display_name
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .or_else(|| self.name.as_ref().and_then(ScimName::display))
    }

    /// The group names this user belongs to, as the mapping matches
    /// them.
    #[must_use]
    pub fn group_names(&self) -> Vec<String> {
        self.groups
            .iter()
            .filter_map(ScimValue::label)
            .map(LdapConfig::group_name)
            .collect()
    }

    /// The primary email, when the provider sent one.
    #[must_use]
    pub fn primary_email(&self) -> Option<&str> {
        self.emails
            .iter()
            .find(|e| e.primary == Some(true))
            .or_else(|| self.emails.first())
            .and_then(|e| e.value.as_deref())
    }
}

#[cfg(test)]
#[path = "scim_user.tests.rs"]
mod tests;
