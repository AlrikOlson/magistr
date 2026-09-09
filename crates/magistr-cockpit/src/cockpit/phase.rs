//! Where the iteration stands, as the engine feed tells it.

use std::fmt;

use magistr_engine::NativeFrame;

/// What a `magistr_set_phase` call is recorded as on the ship trace, before
/// the phase's name.
const SET_PHASE_PREFIX: &str = "workflow phase → ";

/// The workflow phase of the running iteration — research, plan, execute,
/// verify, done — ordered, so the cockpit can refuse to fall back to an
/// earlier phase within one iteration.
///
/// The phase is read off the broadcast frames the think-and-ship engines
/// emit while the agent works ([`Phase::from_frame`]): the roadmap chunk
/// being started, the ship objective set, the first task started, the
/// first check recorded, the objective shipped. A `magistr_set_phase` call
/// is only recorded on the trace while a task is active, so its frame is
/// honoured when it comes but never waited for.
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

    /// The phase `frame` implies, if any.
    #[must_use]
    pub fn from_frame(frame: &NativeFrame) -> Option<Self> {
        match (frame.family.as_str(), frame.kind.as_str()) {
            ("roadmap", "chunk_changed") => (frame.payload["chunk"]["status"].as_str()
                == Some("in_progress"))
            .then_some(Self::Research),
            ("ship", "objective_set") => Some(Self::Plan),
            ("ship", "task_started") => Some(Self::Execute),
            ("ship", "check_recorded") => Some(Self::Verify),
            ("ship", "objective_shipped") => Some(Self::Done),
            ("ship", "action_recorded") => frame.payload["action"]["description"]
                .as_str()
                .and_then(|description| description.strip_prefix(SET_PHASE_PREFIX))
                .and_then(Self::from_name),
            _ => None,
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
