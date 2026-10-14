//! Mutation failures describe whether a durable effect may exist and how a retained
//! observation loses freshness; these decisions share one public failure contract.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ReadError, ReadErrorKind};

/// The boundary reached by a failed mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "read_kind", rename_all = "snake_case")]
pub enum WriteErrorKind {
    Read(ReadErrorKind),
    BeforeCommit,
    Uncertain,
}

/// A mutation never certifies a previous file stamp after this failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WriteError {
    pub kind: WriteErrorKind,
    pub message: String,
}

impl WriteError {
    #[must_use]
    pub fn read(error: ReadError) -> Self {
        Self {
            kind: WriteErrorKind::Read(error.kind),
            message: error.message,
        }
    }

    #[must_use]
    pub fn before_commit(message: impl Into<String>) -> Self {
        Self {
            kind: WriteErrorKind::BeforeCommit,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn uncertain(message: impl Into<String>) -> Self {
        Self {
            kind: WriteErrorKind::Uncertain,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn effect(&self) -> &'static str {
        match self.kind {
            WriteErrorKind::Uncertain => "unknown",
            WriteErrorKind::Read(_) | WriteErrorKind::BeforeCommit => "none",
        }
    }

    #[must_use]
    pub fn as_read_error(&self) -> ReadError {
        ReadError::new(
            match self.kind {
                WriteErrorKind::Read(kind) => kind,
                WriteErrorKind::BeforeCommit => ReadErrorKind::Io,
                WriteErrorKind::Uncertain => ReadErrorKind::Changed,
            },
            self.message.clone(),
        )
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WriteError {}

#[cfg(test)]
#[path = "write_error.tests.rs"]
mod tests;
