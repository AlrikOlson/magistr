//! One entry of a SCIM multi-valued attribute.

use serde::{Deserialize, Serialize};

/// A member of a SCIM multi-valued attribute — an email, a group
/// reference, a phone number.
///
/// RFC 7643 §2.4 gives every multi-valued attribute the same four
/// sub-attributes, which is why one type serves `emails` and `groups`
/// both rather than two that would differ only by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScimValue {
    /// The value itself: the address, or the group's id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// A human-readable rendering — for a group, its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// The entry's type, e.g. `work` or `direct`.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Whether this is the primary entry of its type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
}

impl ScimValue {
    /// The best label for this entry: `display` when the provider sent
    /// one, else the raw `value`.
    ///
    /// A group reference carries the directory's own id in `value` and
    /// the group's name in `display`; a mapping row is written against
    /// the name, so `display` has to win where both are present.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.display
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .or(self.value.as_deref())
            .filter(|v| !v.trim().is_empty())
    }
}
