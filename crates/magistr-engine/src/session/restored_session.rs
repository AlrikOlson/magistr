use crate::roadmap::RoadmapProgress;
use crate::workflow::IterationState;

use super::engine_session::EngineSession;
use super::session_residency::SessionResidency;
use super::session_status::SessionStatus;

/// What came back from a [`SessionSnapshot::restore`], and what did not.
///
/// The three state fields are the session as the new host should carry
/// it. The two report fields are the part a caller must not be allowed
/// to ignore: `severed` names every machine-bound thing the move left
/// behind, and `resumed_from` names the status the snapshot carried when
/// the restore could not honestly keep it.
///
/// `severed` is deliberately not an `Option` and not silently empty on
/// the interesting path: a snapshot taken on a live host always carries
/// at least its agent process, so an empty list means the capture
/// declared nothing, not that nothing was lost.
///
/// [`SessionSnapshot::restore`]: super::session_snapshot::SessionSnapshot::restore
#[derive(Debug, Clone, PartialEq)]
pub struct RestoredSession {
    /// The session, rebuilt with the status the new host can honestly hold.
    pub session: EngineSession,
    /// Where the workflow loop had got to.
    pub iteration: IterationState,
    /// How many iterations in a row had ended without a commit.
    pub consecutive_no_commits: u32,
    /// Roadmap progress as of the capture.
    pub roadmap: RoadmapProgress,
    /// The chunk the session was working on, when it named one.
    pub roadmap_cursor: Option<String>,
    /// Everything the move left on the old machine, each able to say why.
    pub severed: Vec<SessionResidency>,
    /// The status the snapshot carried, when restoring changed it.
    pub resumed_from: Option<SessionStatus>,
}

impl RestoredSession {
    /// Whether the move left anything behind.
    #[must_use]
    pub fn lost_anything(&self) -> bool {
        !self.severed.is_empty()
    }

    /// One line per severed residency, for an operator's log.
    #[must_use]
    pub fn severance_report(&self) -> Vec<String> {
        self.severed
            .iter()
            .map(SessionResidency::describe)
            .collect()
    }
}

#[cfg(test)]
#[path = "restored_session.tests.rs"]
mod tests;
