//! Result of estimating task complexity.

use super::task_complexity::TaskComplexity;

/// Result of estimating task complexity.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexityEstimate {
    /// The estimated complexity level.
    pub complexity: TaskComplexity,
    /// Human-readable reasoning for the estimate.
    pub reasoning: String,
    /// Confidence in the estimate (0.0 to 1.0).
    pub confidence: f64,
}
