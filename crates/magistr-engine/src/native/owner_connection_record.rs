//! One welcomed session as the owner remembers it for its report.

use serde::{Deserialize, Serialize};

use super::attach_kind::AttachKind;
use super::caller_identity::CallerIdentity;

/// A session the owner has welcomed and not yet seen end: what it attached
/// for and who it said it was. The caller identity is the one declared in
/// the `Hello` — kept for diagnostics, proven only by the peer credential
/// the owner checked before welcoming it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerConnectionRecord {
    /// What the session attached for.
    pub attach: AttachKind,
    /// The attaching process as it named itself.
    pub caller: CallerIdentity,
}
