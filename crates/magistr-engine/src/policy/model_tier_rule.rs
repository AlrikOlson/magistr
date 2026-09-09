use serde::{Deserialize, Serialize};

use super::glob_match::glob_match;
use super::model_tier::ModelTier;

/// Which cost tier a family of model ids belongs to.
///
/// The pattern is a glob so a roster can classify a whole family
/// (`claude-opus-*`) rather than chase individual version suffixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTierRule {
    /// Glob over model ids.
    pub pattern: String,
    /// The tier those models cost at.
    pub tier: ModelTier,
}

impl ModelTierRule {
    /// A rule assigning `pattern` to `tier`.
    #[must_use]
    pub fn new(pattern: impl Into<String>, tier: ModelTier) -> Self {
        Self {
            pattern: pattern.into(),
            tier,
        }
    }

    /// Whether this rule classifies `model`.
    #[must_use]
    pub fn matches(&self, model: &str) -> bool {
        glob_match(&self.pattern, model)
    }
}

#[cfg(test)]
#[path = "model_tier_rule.tests.rs"]
mod tests;
