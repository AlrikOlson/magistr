//! Native validation failure, independent of an MCP error envelope.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StepError {
    pub error: String,
    pub status: &'static str,
    pub hint: String,
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.error)
    }
}

impl std::error::Error for StepError {}
