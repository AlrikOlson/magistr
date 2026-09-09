//! A request to compare two models on the same prompt.

/// A request to compare two models on the same prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRequest {
    /// The prompt to send to both models.
    pub prompt: String,
    /// First model ID.
    pub model_a: String,
    /// Second model ID.
    pub model_b: String,
}

impl ComparisonRequest {
    /// Create a new comparison request.
    pub fn new(
        prompt: impl Into<String>,
        model_a: impl Into<String>,
        model_b: impl Into<String>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            model_a: model_a.into(),
            model_b: model_b.into(),
        }
    }

    /// Validate that the two models are different.
    pub fn is_valid(&self) -> bool {
        !self.prompt.is_empty() && self.model_a != self.model_b
    }
}
