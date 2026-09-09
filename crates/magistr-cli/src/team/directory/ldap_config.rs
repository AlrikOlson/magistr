//! Where a team's LDAP directory lives, and how to ask it.

use serde::{Deserialize, Serialize};

/// The LDAP half of a team's `directory` block.
///
/// This describes the query; it does not perform it. The transport is a
/// [`GroupSource`] implementation, and no live one ships yet — what
/// ships here is the part that is wrong in most integrations anyway:
/// building the filter without opening an injection hole, and turning
/// the DNs a `memberOf` attribute answers with back into group names
/// the mapping can match.
///
/// [`GroupSource`]: super::GroupSource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LdapConfig {
    /// The directory URL, e.g. `ldaps://ldap.example.com:636`.
    pub url: String,
    /// The subtree searched for users.
    pub base_dn: String,
    /// The user search filter; `{}` is replaced with the subject.
    #[serde(default = "default_user_filter")]
    pub user_filter: String,
    /// The attribute listing the groups a user belongs to.
    #[serde(default = "default_group_attribute")]
    pub group_attribute: String,
}

fn default_user_filter() -> String {
    "(uid={})".to_string()
}

fn default_group_attribute() -> String {
    "memberOf".to_string()
}

impl LdapConfig {
    /// A config for a URL and a search base, with the defaults for the
    /// rest.
    #[must_use]
    pub fn new(url: impl Into<String>, base_dn: impl Into<String>) -> Self {
        LdapConfig {
            url: url.into(),
            base_dn: base_dn.into(),
            user_filter: default_user_filter(),
            group_attribute: default_group_attribute(),
        }
    }

    /// The search filter for `subject`, with the subject escaped per
    /// RFC 4515 §3.
    ///
    /// The five characters that must be escaped are the ones that would
    /// otherwise let a subject *write filter syntax*: a subject of
    /// `*` turns `(uid={})` into a wildcard that matches every user in
    /// the base DN, and `x)(uid=admin` closes the filter and opens
    /// another. Escaping is why the substitution is a method rather
    /// than a `format!` at the call site.
    #[must_use]
    pub fn filter_for(&self, subject: &str) -> String {
        self.user_filter.replace("{}", &escape_filter(subject))
    }

    /// The group name behind a `memberOf` value.
    ///
    /// Active Directory answers `memberOf` with distinguished names
    /// (`CN=magistr-admins,OU=Groups,DC=example,DC=com`), and a mapping
    /// row is written against the group's own name. The first RDN's
    /// value is that name. A value that is not a DN — OpenLDAP
    /// deployments and SAML attributes both send bare names — is
    /// returned as it came, so one mapping table serves both.
    #[must_use]
    pub fn group_name(value: &str) -> String {
        let first = split_dn_head(value);
        match first.split_once('=') {
            Some((_attr, name)) => unescape_dn(name.trim()),
            None => value.trim().to_string(),
        }
    }
}

/// Escape a value for use inside an LDAP filter (RFC 4515 §3).
///
/// The backslash goes first: escaping it after the others would escape
/// the backslashes they just introduced.
fn escape_filter(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\5c"),
            '*' => out.push_str("\\2a"),
            '(' => out.push_str("\\28"),
            ')' => out.push_str("\\29"),
            '\0' => out.push_str("\\00"),
            other => out.push(other),
        }
    }
    out
}

/// The first RDN of a DN — everything up to the first comma that is not
/// itself escaped.
fn split_dn_head(dn: &str) -> &str {
    let bytes = dn.as_bytes();
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
        } else if b == b'\\' {
            escaped = true;
        } else if b == b',' {
            return dn[..i].trim();
        }
    }
    dn.trim()
}

/// Undo RFC 4514 DN escaping on an attribute value.
fn unescape_dn(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
#[path = "ldap_config.tests.rs"]
mod tests;
