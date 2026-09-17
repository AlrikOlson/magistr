//! One option the agent offers in a permission request.

use alloc::string::String;

use super::permission_kind::PermissionKind;

/// An option of a permission request: the id the agent wants back and
/// what choosing it means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionChoice {
    /// The id the agent gave the option — sent back verbatim.
    pub option_id: String,
    /// What choosing it means.
    pub kind: PermissionKind,
}

impl PermissionChoice {
    /// The option `option_id` of `kind`.
    #[must_use]
    pub fn new(option_id: impl Into<String>, kind: PermissionKind) -> Self {
        Self {
            option_id: option_id.into(),
            kind,
        }
    }
}
