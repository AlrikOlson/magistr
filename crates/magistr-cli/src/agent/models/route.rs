//! Route a prompt to the best available model.

use super::estimate_complexity::estimate_complexity;
use super::model_registry::ModelRegistry;
use super::routing_decision::RoutingDecision;
use super::task_complexity::TaskComplexity;

/// Route a prompt to the best available model.
///
/// If `user_override` is `Some`, it is used directly (no complexity estimation).
/// Otherwise, estimates complexity and picks the best available model.
///
/// Falls back to Sonnet if the ideal model for the complexity level is unavailable.
pub fn route(
    prompt: &str,
    file_count: Option<usize>,
    user_override: Option<&str>,
    registry: &ModelRegistry,
) -> RoutingDecision {
    // User override takes absolute priority
    if let Some(override_id) = user_override {
        let (model_id, display_name) = if let Some(entry) = registry.get(override_id) {
            (entry.config.id.clone(), entry.config.display_name.clone())
        } else {
            // Unknown model — pass through as-is (user knows what they want)
            (override_id.to_string(), override_id.to_string())
        };

        return RoutingDecision {
            model_id,
            display_name,
            reason: "user override (--model flag)".to_string(),
            is_override: true,
            complexity: None,
        };
    }

    // Estimate complexity
    let estimate = estimate_complexity(prompt, file_count);
    let ideal_id = estimate.complexity.default_model_id();

    // Try ideal model, then fall back through the chain.
    // Claude Fable 5 is absent by design — see `TaskComplexity::default_model_id`.
    let fallback_chain: &[&str] = match estimate.complexity {
        TaskComplexity::Simple => &["claude-haiku-4-5", "claude-sonnet-5", "claude-opus-5"],
        TaskComplexity::Moderate => &["claude-sonnet-5", "claude-opus-5", "claude-haiku-4-5"],
        TaskComplexity::Complex => &["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"],
    };

    for &candidate_id in fallback_chain {
        if let Some(entry) = registry.get(candidate_id)
            && entry.config.is_available()
        {
            let reason = if candidate_id == ideal_id {
                format!(
                    "routed to {} for {} task ({})",
                    entry.config.display_name, estimate.complexity, estimate.reasoning
                )
            } else {
                format!(
                    "fallback to {} ({} preferred but unavailable)",
                    entry.config.display_name, ideal_id
                )
            };

            return RoutingDecision {
                model_id: entry.config.id.clone(),
                display_name: entry.config.display_name.clone(),
                reason,
                is_override: false,
                complexity: Some(estimate),
            };
        }
    }

    // Last resort: first available model of any provider
    if let Some(entry) = registry.available().into_iter().next() {
        return RoutingDecision {
            model_id: entry.config.id.clone(),
            display_name: entry.config.display_name.clone(),
            reason: "fallback — no preferred models available".to_string(),
            is_override: false,
            complexity: Some(estimate),
        };
    }

    // Nothing available at all — return ideal and let the caller handle the error
    RoutingDecision {
        model_id: ideal_id.to_string(),
        display_name: ideal_id.to_string(),
        reason: "no models available — routing to ideal model anyway".to_string(),
        is_override: false,
        complexity: Some(estimate),
    }
}
