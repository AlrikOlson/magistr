//! A routing decision — which model to use and why.

use super::complexity_estimate::ComplexityEstimate;

/// A routing decision — which model to use and why.
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingDecision {
    /// The chosen model ID.
    pub model_id: String,
    /// The chosen model's display name.
    pub display_name: String,
    /// Why this model was chosen.
    pub reason: String,
    /// Whether this was a user override (--model flag).
    pub is_override: bool,
    /// The complexity estimate (if routing was performed, not overridden).
    pub complexity: Option<ComplexityEstimate>,
}
