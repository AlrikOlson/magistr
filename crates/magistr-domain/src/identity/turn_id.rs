use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::{IdentityError, SessionId, identity_key::IdentityKey};

/// A turn key scoped to one Magistr session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TurnId {
    session: SessionId,
    local: IdentityKey,
}

impl TurnId {
    pub fn new(session: SessionId, key: impl Into<String>) -> Result<Self, IdentityError> {
        Ok(Self {
            session,
            local: IdentityKey::new(key)?,
        })
    }

    pub fn session(&self) -> &SessionId {
        &self.session
    }

    pub fn local(&self) -> &str {
        self.local.as_str()
    }
}
