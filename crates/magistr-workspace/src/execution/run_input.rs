use crate::contracts::{ErrorKind, WorkspaceError};

/// One declared-command request: the `[commands]` key, extra argv appended
/// literally, a wall-clock limit in seconds (retained default 300, clamped to
/// `1..=3600`) and the response budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunInput {
    pub command: String,
    pub args: Vec<String>,
    pub timeout_secs: u64,
    pub budget: u64,
}

impl RunInput {
    pub const TIMEOUT_DEFAULT_SECS: u64 = 300;
    pub const TIMEOUT_MAX_SECS: u64 = 3600;
    pub const BUDGET_DEFAULT: u64 = 4000;

    #[must_use]
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            timeout_secs: Self::TIMEOUT_DEFAULT_SECS,
            budget: Self::BUDGET_DEFAULT,
        }
    }

    #[must_use]
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn with_timeout_secs(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    #[must_use]
    pub fn with_budget(mut self, budget: u64) -> Self {
        self.budget = budget;
        self
    }

    /// Schema-level misuse is refused before admission, as the retained tool
    /// threw before producing an envelope; natively it is a typed narrow error.
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.command.is_empty() {
            return Err(WorkspaceError::new(
                ErrorKind::PatternInvalid,
                "run: command must be a non-empty string",
            ));
        }
        if self.timeout_secs == 0 || self.timeout_secs > Self::TIMEOUT_MAX_SECS {
            return Err(WorkspaceError::new(
                ErrorKind::PatternInvalid,
                format!(
                    "run: timeout must be an integer in [1, {}] seconds",
                    Self::TIMEOUT_MAX_SECS
                ),
            ));
        }
        if self.budget == 0 {
            return Err(WorkspaceError::new(
                ErrorKind::BudgetTooSmall,
                "run: budget must be a positive integer",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "run_input.tests.rs"]
mod tests;
