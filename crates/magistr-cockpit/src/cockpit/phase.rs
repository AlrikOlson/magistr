//! Where the iteration stands, as the engines' typed events tell it.

use std::fmt;

use magistr_engine::native::ChunkStatus;
use magistr_engine::{MagistrFrame, NativeEvent, RoadmapFrame, ShipFrame, WorkflowPhase};

/// The workflow phase of the running iteration — research, plan, execute,
/// verify, done — ordered, so the cockpit can refuse to fall back to an
/// earlier phase within one iteration.
///
/// The phase is read off the typed events the engines emit while the
/// agent works ([`Phase::from_event`]): the roadmap chunk being started,
/// the ship objective set, the first task started, the first check
/// recorded, the objective shipped — and the phase the agent announced
/// through `magistr_set_phase`, which the host publishes typed
/// ([`MagistrFrame::PhaseSet`]) and which is honoured when it comes but
/// never waited for. Nothing is read out of an action's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// The chunk is being read and the world researched.
    Research,
    /// The objective and its plan are being written.
    Plan,
    /// Code is being written.
    Execute,
    /// The gates are running.
    Verify,
    /// The objective shipped.
    Done,
}

impl Phase {
    /// Every phase, in order.
    pub const ALL: [Self; 5] = [
        Self::Research,
        Self::Plan,
        Self::Execute,
        Self::Verify,
        Self::Done,
    ];

    /// The phase's name, as `magistr_set_phase` takes it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Plan => "plan",
            Self::Execute => "execute",
            Self::Verify => "verify",
            Self::Done => "done",
        }
    }

    /// The phase named `name` (case-insensitive, surrounding blanks
    /// ignored), if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|phase| phase.label().eq_ignore_ascii_case(name))
    }

    /// The phase `event` implies, if any.
    #[must_use]
    pub fn from_event(event: &NativeEvent) -> Option<Self> {
        match event {
            NativeEvent::Roadmap(RoadmapFrame::ChunkChanged { chunk }) => {
                (chunk.status == ChunkStatus::InProgress).then_some(Self::Research)
            }
            NativeEvent::Ship(ShipFrame::ObjectiveSet { .. }) => Some(Self::Plan),
            NativeEvent::Ship(ShipFrame::TaskStarted { .. }) => Some(Self::Execute),
            NativeEvent::Ship(ShipFrame::CheckRecorded { .. }) => Some(Self::Verify),
            NativeEvent::Ship(ShipFrame::ObjectiveShipped { .. }) => Some(Self::Done),
            NativeEvent::Magistr(MagistrFrame::PhaseSet { phase }) => Some(Self::from(*phase)),
            _ => None,
        }
    }
}

impl From<WorkflowPhase> for Phase {
    fn from(phase: WorkflowPhase) -> Self {
        match phase {
            WorkflowPhase::Researching => Self::Research,
            WorkflowPhase::Planning => Self::Plan,
            WorkflowPhase::Executing => Self::Execute,
            WorkflowPhase::Verifying => Self::Verify,
            WorkflowPhase::Complete => Self::Done,
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
#[path = "phase.tests.rs"]
mod tests;
