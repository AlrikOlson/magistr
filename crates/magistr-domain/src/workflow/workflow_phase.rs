use core::fmt;
use serde::{Deserialize, Serialize};

/// The phase of the research-plan-execute-verify workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    /// Gathering information: web search, doc reading, codebase exploration.
    Researching,
    /// Writing the implementation plan.
    Planning,
    /// Implementing the plan with TDD.
    Executing,
    /// Running quality gates, updating spec, committing.
    Verifying,
    /// Iteration complete — committed and cleaned up.
    Complete,
}

impl WorkflowPhase {
    /// Human-readable label for this phase.
    pub fn label(self) -> &'static str {
        match self {
            Self::Researching => "Researching",
            Self::Planning => "Planning",
            Self::Executing => "Executing",
            Self::Verifying => "Verifying",
            Self::Complete => "Complete",
        }
    }

    /// Index of this phase in the workflow (0-based).
    ///
    /// Researching=0, Planning=1, Executing=2, Verifying=3, Complete=4.
    pub fn step_index(self) -> usize {
        match self {
            Self::Researching => 0,
            Self::Planning => 1,
            Self::Executing => 2,
            Self::Verifying => 3,
            Self::Complete => 4,
        }
    }

    /// Parse a phase name from a string (case-insensitive).
    ///
    /// Accepts both short names ("research", "plan") and present-participle
    /// forms ("researching", "planning").
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "research" | "researching" => Some(Self::Researching),
            "plan" | "planning" => Some(Self::Planning),
            "execute" | "executing" => Some(Self::Executing),
            "verify" | "verifying" => Some(Self::Verifying),
            "complete" | "done" => Some(Self::Complete),
            _ => None,
        }
    }

    /// Whether this is a terminal phase.
    pub fn is_terminal(self) -> bool {
        self == Self::Complete
    }

    /// Whether a transition may advance to `to`.
    ///
    /// Re-entering the same phase and skipping forward are permitted.
    /// A complete iteration may restart at any phase during recovery.
    #[must_use]
    pub fn can_advance_to(self, to: Self) -> bool {
        self.is_terminal() || to.step_index() >= self.step_index()
    }

    /// All phases in workflow order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Researching,
            Self::Planning,
            Self::Executing,
            Self::Verifying,
            Self::Complete,
        ]
    }
}

impl fmt::Display for WorkflowPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
#[path = "workflow_phase.tests.rs"]
mod tests;
