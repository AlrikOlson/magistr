//! Exit code conventions for headless/CI mode.

use std::fmt;

/// Process exit codes for headless mode.
///
/// These codes communicate the result of a headless session to CI pipelines
/// and shell scripts without requiring output parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    /// All gates passed, session completed successfully.
    Success = 0,
    /// One or more quality gates failed.
    GatesFailed = 1,
    /// A runtime error occurred (agent crash, network failure, etc.).
    RuntimeError = 2,
    /// The spending budget was exceeded.
    BudgetExceeded = 3,
}

impl ExitCode {
    /// Whether this exit code represents success.
    #[must_use]
    pub fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        Self::from(code as u8)
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "success (0)"),
            Self::GatesFailed => write!(f, "gates failed (1)"),
            Self::RuntimeError => write!(f, "runtime error (2)"),
            Self::BudgetExceeded => write!(f, "budget exceeded (3)"),
        }
    }
}

#[cfg(test)]
#[path = "exit_code.tests.rs"]
mod tests;
