//! Application workflow phase — lifecycle of a Magistr session.

/// The current phase of the application workflow.
///
/// Tracks the high-level state of the Magistr session from initial stack
/// detection through agent execution to completion. Frontend renderers
/// (desktop, CLI) map these variants to visual indicators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    /// Detecting project stack
    Detecting,
    /// Loading preset configuration
    LoadingPreset,
    /// Interactive wizard interview
    Wizard,
    /// Generating scaffold files
    Scaffolding,
    /// Connecting to the agent backend (spawning process, handshake)
    Connecting,
    /// Running an agent session (streaming output)
    Running,
    /// Waiting for follow-up input from the user
    AwaitingInput,
    /// Agent is winding down after cancel request (ACP graceful cancel)
    Cancelling,
    /// All done
    Done,
    /// An error occurred
    Error(String),
    /// Interactive roadmap wizard
    Roadmap,
}

impl Phase {
    /// Human-readable label for this phase.
    pub fn label(&self) -> &str {
        match self {
            Self::Detecting => "Detecting stack...",
            Self::LoadingPreset => "Loading preset...",
            Self::Wizard => "Configure project...",
            Self::Roadmap => "Interactive roadmap...",
            Self::Scaffolding => "Scaffolding .claude/...",
            Self::Connecting => "Connecting to agent...",
            Self::Running => "Running agent...",
            Self::AwaitingInput => "Enter follow-up or Esc to quit",
            Self::Cancelling => "Cancelling...",
            Self::Done => "Done",
            Self::Error(_) => "Error",
        }
    }

    /// Whether this phase represents a terminal state (done or error).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done | Self::Error(_))
    }

    /// Whether this is a loading/connecting phase (skeleton shimmer should be active).
    pub fn is_loading(&self) -> bool {
        matches!(
            self,
            Self::Detecting
                | Self::LoadingPreset
                | Self::Scaffolding
                | Self::Connecting
                | Self::Running
        )
    }
}

#[cfg(test)]
#[path = "phase.tests.rs"]
mod tests;
