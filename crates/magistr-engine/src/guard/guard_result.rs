use super::guard_action::GuardAction;

/// The result of a guard check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardResult {
    /// Which guard produced this result.
    pub guard_name: String,
    /// What the orchestrator should do.
    pub action: GuardAction,
}

impl GuardResult {
    /// Create a `Continue` result.
    pub fn ok(guard_name: impl Into<String>) -> Self {
        Self {
            guard_name: guard_name.into(),
            action: GuardAction::Continue,
        }
    }

    /// Create a `SteerPrompt` result.
    pub fn steer(guard_name: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            guard_name: guard_name.into(),
            action: GuardAction::SteerPrompt(prompt.into()),
        }
    }

    /// Create a `Warn` result.
    pub fn warn(guard_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            guard_name: guard_name.into(),
            action: GuardAction::Warn(message.into()),
        }
    }

    /// Whether this result requires no action from the orchestrator.
    pub fn is_ok(&self) -> bool {
        matches!(self.action, GuardAction::Continue)
    }
}
