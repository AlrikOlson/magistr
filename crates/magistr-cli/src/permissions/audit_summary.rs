//! Summary statistics for audit entries.

/// Summary statistics for audit entries.
#[derive(Debug, Clone)]
pub struct AuditSummary {
    /// Total entries.
    pub total: usize,
    /// Number of allowed entries.
    pub allowed: usize,
    /// Number of denied entries.
    pub denied: usize,
    /// Most frequently allowed tools.
    pub top_allowed_tools: Vec<(String, usize)>,
}
