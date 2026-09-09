//! What a SCIM request did to the roster.

use std::fmt;

/// The effect one [`ScimRequest`] had.
///
/// [`Deactivated`] and [`Removed`] are kept apart on purpose. SCIM's
/// own deprovisioning signal is a soft delete — `active: false` — and
/// the row stays, so a later reactivation is one field rather than a
/// re-provision; an actual `DELETE` is a hard delete and the row goes.
/// Reporting both as "gone" would hide which of the two happened from
/// the operator reading the output.
///
/// [`ScimRequest`]: super::ScimRequest
/// [`Deactivated`]: ScimEffect::Deactivated
/// [`Removed`]: ScimEffect::Removed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScimEffect {
    /// A new roster row was written.
    Created,
    /// An existing row's name or role changed.
    Updated,
    /// A provisioned row was deactivated (`active: false`).
    Deactivated,
    /// A deactivated row was provisioned again.
    Reactivated,
    /// The row was deleted outright.
    Removed,
    /// The request asked for what the roster already said.
    Unchanged,
}

impl fmt::Display for ScimEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ScimEffect::Created => "created",
            ScimEffect::Updated => "updated",
            ScimEffect::Deactivated => "deactivated",
            ScimEffect::Reactivated => "reactivated",
            ScimEffect::Removed => "removed",
            ScimEffect::Unchanged => "unchanged",
        };
        f.write_str(s)
    }
}
