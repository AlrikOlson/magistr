/// What a guard wants the orchestrator to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardAction {
    /// Everything is fine, proceed normally.
    Continue,
    /// Steer the agent's next prompt with additional guidance.
    SteerPrompt(String),
    /// Emit a warning but don't change behavior.
    Warn(String),
}
