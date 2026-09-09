//! Orchestration preset — defines how multiple agent roles collaborate.

use std::fmt;

use serde::{Deserialize, Serialize};

/// How multiple agent roles collaborate on a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrchestrationPreset {
    /// Sequential: researcher gathers context, then coder implements.
    ResearchThenImplement,
    /// Pair: coder writes code, reviewer critiques.
    ReviewPair,
    /// Parallel with TDD: 3+ roles work concurrently with test gates.
    ParallelTdd,
}

impl OrchestrationPreset {
    /// Infer a preset from a set of role IDs.
    ///
    /// Rules:
    /// - researcher + coder (exactly 2) → `ResearchThenImplement`
    /// - coder + reviewer (exactly 2) → `ReviewPair`
    /// - 3+ roles or ambiguous 2-role combo → `ParallelTdd`
    #[must_use]
    pub fn infer(roles: &[impl AsRef<str>]) -> Self {
        if roles.len() == 2 {
            let has = |id: &str| roles.iter().any(|r| r.as_ref() == id);
            if has("researcher") && has("coder") {
                return Self::ResearchThenImplement;
            }
            if has("coder") && has("reviewer") {
                return Self::ReviewPair;
            }
        }
        Self::ParallelTdd
    }
}

impl fmt::Display for OrchestrationPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResearchThenImplement => write!(f, "research-then-implement"),
            Self::ReviewPair => write!(f, "review-pair"),
            Self::ParallelTdd => write!(f, "parallel-tdd"),
        }
    }
}

#[cfg(test)]
#[path = "orchestration_preset.tests.rs"]
mod tests;
