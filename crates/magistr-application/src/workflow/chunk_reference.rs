use alloc::string::String;
use magistr_domain::identity::ProjectId;

use crate::ApplicationError;

/// A native chunk key scoped to its project. Existence is checked by the port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkReference {
    project: ProjectId,
    key: String,
}

impl ChunkReference {
    pub fn new(project: ProjectId, key: impl Into<String>) -> Result<Self, ApplicationError> {
        let key = key.into();
        super::validate_key(&key)?;
        Ok(Self { project, key })
    }

    pub fn project(&self) -> &ProjectId {
        &self.project
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}
