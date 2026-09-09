use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::{IdentityError, identity_key::IdentityKey};

/// Magistr project identity supplied by the project authority, not a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(IdentityKey);

impl ProjectId {
    pub fn new(key: impl Into<String>) -> Result<Self, IdentityError> {
        IdentityKey::new(key).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
