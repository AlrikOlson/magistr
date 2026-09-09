use alloc::string::String;
use serde::{Deserialize, Serialize};

use super::{IdentityError, ProjectId, identity_key::IdentityKey};

/// A run key scoped to its Magistr project.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RunId {
    project: ProjectId,
    local: IdentityKey,
}

impl RunId {
    pub fn new(project: ProjectId, key: impl Into<String>) -> Result<Self, IdentityError> {
        Ok(Self {
            project,
            local: IdentityKey::new(key)?,
        })
    }

    pub fn project(&self) -> &ProjectId {
        &self.project
    }

    pub fn local(&self) -> &str {
        self.local.as_str()
    }
}
