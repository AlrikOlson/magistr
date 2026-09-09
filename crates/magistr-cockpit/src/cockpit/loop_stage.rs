//! Where the loop driver stands within an iteration.

use std::fmt;

/// The loop driver's stage within an iteration, as it reports it
/// ([`LoopEvent::Stage`](super::LoopEvent::Stage)): the window's stage
/// cap reads it, and the between-agent stages are what keep
/// [`Model::is_working`](super::Model::is_working) true while no turn
/// is streaming.
///
/// The stages exist because the engine feed dies with every agent —
/// its socket is bound by the agent's own `magistr mcp-serve` — so a
/// cockpit that only knew the feed would read every gap between agents
/// as a lost connection (`loop-gap-not-a-fault`). The driver knows what
/// it is doing in the gap; this is it saying so.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoopStage {
    /// The agent's transport is being spawned and handshaken.
    Connecting,
    /// The agent's turn is running — the stage a status starts in, so
    /// a loop that never names its stage reads with the default stage label.
    #[default]
    Agent,
    /// The gates run as jobs after the turn.
    Gates,
    /// The judge is asked about the iteration's chunk.
    Judge,
}

impl LoopStage {
    /// The stage in words, for a cap or a strip.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connecting => "starting the agent",
            Self::Agent => "agent working",
            Self::Gates => "gates running",
            Self::Judge => "judge deciding",
        }
    }

    /// Whether the driver is between agents: no turn can stream, and
    /// the feed's socket is expected to be gone.
    #[must_use]
    pub const fn is_between_agents(self) -> bool {
        !matches!(self, Self::Agent)
    }
}

impl fmt::Display for LoopStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
#[path = "loop_stage.tests.rs"]
mod tests;
