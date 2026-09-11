use std::path::PathBuf;

use crate::{
    contracts::{ErrorKind, WorkspaceError},
    index::IndexError,
    jail::KeyError,
    journal::JournalError,
    worker::WorkerError,
};

/// Why a workspace could not be admitted. Each variant maps onto the
/// retained process exit codes, so a host answers a refusal the way the
/// reference did — before any transport or negotiated request exists.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceOpenError {
    #[error("workspace root does not exist: {}", .0.display())]
    RootMissing(PathBuf),
    #[error("{0}")]
    Locked(String),
    /// Durable data written by a newer binary: fail closed rather than guess.
    #[error("{0}")]
    UnsupportedVersion(String),
    #[error("{0}")]
    Policy(WorkspaceError),
    #[error("workspace key: {0}")]
    Key(#[from] KeyError),
    #[error("journal: {0}")]
    Journal(JournalError),
    #[error("worker: {0}")]
    Worker(#[from] WorkerError),
    #[error("index: {0}")]
    Index(#[from] IndexError),
    #[error("workspace I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl WorkspaceOpenError {
    /// EX_DATAERR: durable data this binary cannot read.
    pub const EXIT_DATA_ERROR: i32 = 65;
    /// EX_NOINPUT: the workspace root is missing.
    pub const EXIT_NO_INPUT: i32 = 66;
    /// EX_TEMPFAIL: another live instance owns the workspace; retry later.
    pub const EXIT_LOCKED: i32 = 75;

    /// The retained exit code for this refusal; every other failure is 1.
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::RootMissing(_) => Self::EXIT_NO_INPUT,
            Self::Locked(_) => Self::EXIT_LOCKED,
            Self::UnsupportedVersion(_) => Self::EXIT_DATA_ERROR,
            _ => 1,
        }
    }

    pub(super) fn policy(error: WorkspaceError) -> Self {
        if error.kind == ErrorKind::UnsupportedVersion {
            Self::UnsupportedVersion(error.message)
        } else {
            Self::Policy(error)
        }
    }

    pub(super) fn journal(error: JournalError) -> Self {
        match error {
            JournalError::UnsupportedVersion(detail) => Self::UnsupportedVersion(format!(
                "the journal cannot be read by this binary: {detail}; upgrade magistr — durable data was left untouched"
            )),
            other => Self::Journal(other),
        }
    }

    pub(super) fn lock(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            Self::Locked(error.to_string())
        } else {
            Self::Io(error)
        }
    }
}
