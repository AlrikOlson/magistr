use super::{ErrorKind, Recovery};

/// Native failure with typed payload and a contextual human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceError {
    pub kind: ErrorKind,
    pub message: String,
}

impl WorkspaceError {
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }

    #[must_use]
    pub const fn recovery(&self) -> Recovery {
        self.kind.recovery()
    }
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message)
    }
}

impl std::error::Error for WorkspaceError {}
