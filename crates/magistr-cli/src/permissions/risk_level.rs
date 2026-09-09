//! Risk level for tool calls and risk classification.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Risk level for a tool call.
///
/// Classified based on tool name and input parameters:
/// - **Low**: read-only operations (Read, Search, Grep, Glob)
/// - **Medium**: file modifications (Write, Edit) — reversible
/// - **High**: potentially destructive (Bash with rm/push, network access)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// Read-only, informational operations.
    Low,
    /// File changes — reversible but impactful.
    Medium,
    /// Potentially destructive or irreversible operations.
    High,
}

impl RiskLevel {
    /// Emoji badge for display.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Low => "🟢",
            Self::Medium => "🟡",
            Self::High => "🔴",
        }
    }

    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}
