//! Role recommendation result — the assembled output of role recommendation
//! and preset inference, ready for serialization to frontends.

use serde::{Deserialize, Serialize};

use crate::orchestration_preset::OrchestrationPreset;

/// Complete role recommendation result.
///
/// Combines the recommended role IDs, a human-readable description,
/// and the inferred orchestration preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleRecommendation {
    /// Recommended role IDs (e.g., `["researcher", "coder", "tester"]`).
    pub role_ids: Vec<String>,
    /// Human-readable description of why these roles were chosen.
    pub description: String,
    /// Suggested orchestration preset for these roles.
    pub suggested_preset: OrchestrationPreset,
}

impl RoleRecommendation {
    /// Build a recommendation from a task description.
    ///
    /// Runs role recommendation and preset inference in one step.
    #[must_use]
    pub fn for_task(task: &str) -> Self {
        let (role_ids, description) = crate::role_recommend::recommend_roles_with_description(task);
        let role_ids: Vec<String> = role_ids.into_iter().map(String::from).collect();
        let suggested_preset = OrchestrationPreset::infer(&role_ids);
        Self {
            role_ids,
            description,
            suggested_preset,
        }
    }
}

#[cfg(test)]
#[path = "role_recommendation.tests.rs"]
mod tests;
