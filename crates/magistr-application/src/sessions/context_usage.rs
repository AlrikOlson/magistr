//! Reported context measurements travel together, independently of spending.

/// The token counts the agent reported for its current context window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextUsage {
    /// Tokens in the window.
    pub used: u64,
    /// The window's capacity.
    pub size: u64,
}
