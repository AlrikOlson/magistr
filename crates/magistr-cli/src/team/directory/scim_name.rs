//! The SCIM `name` complex attribute.

use serde::{Deserialize, Serialize};

/// A user's name, as SCIM breaks it up (RFC 7643 §4.1.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimName {
    /// The full name as the provider would print it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatted: Option<String>,
    /// The given name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    /// The family name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
}

impl ScimName {
    /// The display name these parts make, if any.
    #[must_use]
    pub fn display(&self) -> Option<String> {
        if let Some(formatted) = self.formatted.as_deref().filter(|f| !f.trim().is_empty()) {
            return Some(formatted.to_string());
        }
        let joined = [self.given_name.as_deref(), self.family_name.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!joined.is_empty()).then_some(joined)
    }
}
