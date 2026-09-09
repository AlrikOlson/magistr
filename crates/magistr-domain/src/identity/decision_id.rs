use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::{IdentityError, TurnId, identity_key::IdentityKey};

/// A permission or policy decision key scoped to the requesting turn.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DecisionId {
    turn: TurnId,
    local: IdentityKey,
}

impl DecisionId {
    pub fn new(turn: TurnId, key: impl Into<String>) -> Result<Self, IdentityError> {
        Ok(Self {
            turn,
            local: IdentityKey::new(key)?,
        })
    }

    pub fn turn(&self) -> &TurnId {
        &self.turn
    }

    pub fn local(&self) -> &str {
        self.local.as_str()
    }
}
