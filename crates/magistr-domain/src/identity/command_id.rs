use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::{IdentityError, RunId, identity_key::IdentityKey};

/// An application command key scoped to one run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CommandId {
    run: RunId,
    local: IdentityKey,
}

impl CommandId {
    pub fn new(run: RunId, key: impl Into<String>) -> Result<Self, IdentityError> {
        Ok(Self {
            run,
            local: IdentityKey::new(key)?,
        })
    }

    pub fn run(&self) -> &RunId {
        &self.run
    }

    pub fn local(&self) -> &str {
        self.local.as_str()
    }
}
