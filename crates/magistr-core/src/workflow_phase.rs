//! Workflow phase tracking — which phase of the iteration lifecycle the agent is in.

/// Which phase of the Research-Plan-Execute-Verify iteration the agent is in.
///
/// Only active during workflow runs (`magistr next` / `magistr next-loop`).
/// Generic `magistr run` sessions leave this as `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowPhase {
    /// Agent is researching via web search / firecrawl before planning.
    Researching,
    /// Agent is reading codebase and writing plan file.
    Planning,
    /// Agent is implementing (TDD: red-green-refactor).
    Executing,
    /// Agent is running quality gates, updating spec, committing.
    Verifying,
    /// Iteration complete.
    Complete,
}

impl WorkflowPhase {
    /// Human-readable label for this phase.
    pub fn label(self) -> &'static str {
        match self {
            Self::Researching => "Researching...",
            Self::Planning => "Planning...",
            Self::Executing => "Executing...",
            Self::Verifying => "Verifying...",
            Self::Complete => "Complete",
        }
    }

    /// Index of this phase in the 4-step progress (0-based).
    ///
    /// Returns: Researching=0, Planning=1, Executing=2, Verifying=3, Complete=4.
    /// Complete returns 4 so the progress bar can advance past all 4 steps.
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
    /// forms ("researching", "planning") for flexibility in tool inputs.
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "research" | "researching" => Some(Self::Researching),
            "plan" | "planning" => Some(Self::Planning),
            "execute" | "executing" => Some(Self::Executing),
            "verify" | "verifying" => Some(Self::Verifying),
            "done" | "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

impl std::fmt::Display for WorkflowPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
#[path = "workflow_phase.tests.rs"]
mod tests;
