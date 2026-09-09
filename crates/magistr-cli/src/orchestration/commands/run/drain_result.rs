/// Result of draining a single agent turn.
#[derive(Debug, Clone)]
pub struct DrainResult {
    /// Collected assistant text output.
    pub text: String,
    /// Whether the agent ended with an error.
    pub is_error: bool,
    /// Number of turns completed.
    pub num_turns: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Total cost in USD (if reported).
    pub total_cost_usd: Option<f64>,
    /// Session ID from the result message.
    pub session_id: String,
}
