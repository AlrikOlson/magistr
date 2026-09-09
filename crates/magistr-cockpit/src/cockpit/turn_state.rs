//! Where the turn stands.

use std::fmt;

/// Whether the agent is talking, done, or has not started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TurnState {
    /// Nothing has arrived yet.
    #[default]
    Idle,
    /// Deltas are arriving.
    Streaming,
    /// The turn ended; the next prompt is the user's.
    AwaitingInput,
}

impl TurnState {
    /// The state in words, for the header and the summary.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Streaming => "streaming",
            Self::AwaitingInput => "awaiting input",
        }
    }
}

impl fmt::Display for TurnState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
