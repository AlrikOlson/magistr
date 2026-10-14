use std::fmt;

use magistr_workflow_contracts::{ReadError, WriteError, WriteErrorKind};

/// A refused operation or a storage failure with an explicit effect boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShipError {
    Refused { kind: &'static str, message: String },
    Store(WriteError),
}

impl ShipError {
    pub(crate) fn invalid_args(message: impl Into<String>) -> Self {
        Self::Refused {
            kind: "invalid_args",
            message: message.into(),
        }
    }

    pub(crate) fn refused(kind: &'static str, message: impl Into<String>) -> Self {
        Self::Refused {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Refused { kind, .. } => kind,
            Self::Store(error) => match error.kind {
                WriteErrorKind::Read(_) => "read_failed",
                WriteErrorKind::BeforeCommit => "write_failed",
                WriteErrorKind::Uncertain => "write_uncertain",
            },
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Refused { message, .. } => message,
            Self::Store(error) => &error.message,
        }
    }

    #[must_use]
    pub fn effect(&self) -> &'static str {
        match self {
            Self::Refused { .. } => "none",
            Self::Store(error) => error.effect(),
        }
    }

    #[must_use]
    pub fn store_error(&self) -> Option<&WriteError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    pub(crate) fn after_commit(self) -> Self {
        Self::Store(WriteError::uncertain(self.to_string()))
    }
}

impl From<String> for ShipError {
    fn from(message: String) -> Self {
        Self::refused("invalid_state", message)
    }
}

impl From<&str> for ShipError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

impl From<WriteError> for ShipError {
    fn from(error: WriteError) -> Self {
        Self::Store(error)
    }
}

impl From<ReadError> for ShipError {
    fn from(error: ReadError) -> Self {
        Self::Store(WriteError::read(error))
    }
}

impl fmt::Display for ShipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for ShipError {}
