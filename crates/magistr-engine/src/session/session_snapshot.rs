use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::roadmap::RoadmapProgress;
use crate::workflow::IterationState;

use super::engine_session::EngineSession;
use super::repo_state::RepoState;
use super::restored_session::RestoredSession;
use super::session_cost::SessionCost;
use super::session_residency::SessionResidency;
use super::session_status::SessionStatus;

/// A session written down: everything about it that is a *value*, plus
/// an honest list of everything about it that is not.
///
/// A session that only lives in one process cannot survive that process
/// draining, restarting, or being taken out for maintenance. This is the
/// document that lets it: status, phase, iteration, cost and the roadmap
/// cursor are values, and a value crosses a machine for free. What does
/// not cross is declared in [`residencies`] — see [`SessionResidency`]
/// for why that is a list rather than an omission.
///
/// Three rules make the document safe to read on the other side.
///
/// **The format is versioned and never guessed at.** [`restore`] refuses
/// a `version` it does not know rather than parsing what it can. A
/// persistence layer that silently accepts a record from a schema it has
/// not been taught reintroduces exactly the hazard versioning exists to
/// name, so evolution here means an explicit migration step, not a
/// best-effort read.
///
/// **A status that asserts something about a live process cannot
/// survive the move.** `InProgress` means "an agent is thinking right
/// now"; on the second machine none is, because the process that was
/// thinking stayed behind. So [`restore`] lands it at
/// [`SessionStatus::Awaiting`] — the turn boundary — and records what it
/// came from. Serialization, by contrast, keeps the status exactly:
/// round-tripping a snapshot is not restoring it, and conflating the two
/// would make the document lie about the machine it was taken on.
///
/// **Capture is a value, not a handle.** Nothing here borrows the
/// engine. The engine cannot know about ACP children or terminal ids, so
/// [`capture`] takes what the engine has and the adapter that *does*
/// know adds its residencies with [`resident_on`].
///
/// [`residencies`]: SessionSnapshot::residencies
/// [`restore`]: SessionSnapshot::restore
/// [`capture`]: SessionSnapshot::capture
/// [`resident_on`]: SessionSnapshot::resident_on
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// The snapshot format this document was written in.
    pub version: u32,
    /// The session's id, which it keeps across the move.
    pub session_id: String,
    /// The status as of the capture — not necessarily what a restore
    /// can hold. See [`SessionSnapshot::restore`].
    pub status: SessionStatus,
    /// The workflow phase the session had reached.
    pub phase: crate::workflow::WorkflowPhase,
    /// Where the workflow loop had got to.
    pub iteration: IterationState,
    /// How many iterations in a row have ended without a commit.
    ///
    /// Carried because it is a *stop condition*, not a statistic: a loop
    /// two no-commits deep is one iteration from stopping, and a
    /// migration that dropped the count would let a wedged loop launder
    /// its own stop by moving machines.
    #[serde(default)]
    pub consecutive_no_commits: u32,
    /// Accumulated cost and token usage.
    pub cost: SessionCost,
    /// Roadmap progress as of the capture.
    #[serde(default)]
    pub roadmap: RoadmapProgress,
    /// The chunk the session was working on, when it named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roadmap_cursor: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When this snapshot was taken.
    pub captured_at: DateTime<Utc>,
    /// The session's message log.
    #[serde(default)]
    pub messages: Vec<String>,
    /// The repository the session was running in, when the adapter
    /// knew one.
    ///
    /// A *value*, not a residency: a sha and a branch name cross a
    /// machine for free. Carrying it is what lets a restore say which
    /// commit the workflow state it is holding was produced against —
    /// and what lets a fork start a branch there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<RepoState>,
    /// Everything the session holds that is bound to its machine.
    #[serde(default)]
    pub residencies: Vec<SessionResidency>,
}

impl SessionSnapshot {
    /// The snapshot format this build writes and is the only one it
    /// reads. A document from any other version is refused rather than
    /// interpreted.
    pub const VERSION: u32 = 1;

    /// Write down a session and where its workflow loop had got to.
    ///
    /// Declares no residencies: the engine does not know about child
    /// processes or terminal ids. The adapter that does adds them with
    /// [`resident_on`].
    ///
    /// [`resident_on`]: SessionSnapshot::resident_on
    #[must_use]
    pub fn capture(
        session: &EngineSession,
        iteration: IterationState,
        captured_at: DateTime<Utc>,
    ) -> Self {
        Self {
            version: Self::VERSION,
            session_id: session.session_id().to_owned(),
            status: session.status(),
            phase: session.phase(),
            iteration,
            consecutive_no_commits: 0,
            cost: session.cost().clone(),
            roadmap: RoadmapProgress::default(),
            roadmap_cursor: None,
            created_at: session.created_at(),
            captured_at,
            messages: session.messages().to_vec(),
            repo: None,
            residencies: Vec::new(),
        }
    }

    /// Record the repository the session was running in.
    ///
    /// The engine never reads git, so this is the adapter's to supply —
    /// the same division [`resident_on`] draws.
    ///
    /// [`resident_on`]: SessionSnapshot::resident_on
    #[must_use]
    pub fn in_repo(mut self, repo: RepoState) -> Self {
        self.repo = Some(repo);
        self
    }

    /// Record how many iterations in a row have ended without a commit.
    #[must_use]
    pub fn with_no_commits(mut self, consecutive_no_commits: u32) -> Self {
        self.consecutive_no_commits = consecutive_no_commits;
        self
    }

    /// Record roadmap progress and the chunk being worked on.
    #[must_use]
    pub fn with_roadmap(mut self, roadmap: RoadmapProgress, cursor: Option<String>) -> Self {
        self.roadmap = roadmap;
        self.roadmap_cursor = cursor;
        self
    }

    /// Declare what the session holds that is bound to this machine.
    #[must_use]
    pub fn resident_on(mut self, residencies: Vec<SessionResidency>) -> Self {
        self.residencies = residencies;
        self
    }

    /// Declare one more machine-bound thing.
    #[must_use]
    pub fn holding(mut self, residency: SessionResidency) -> Self {
        self.residencies.push(residency);
        self
    }

    /// The residencies that stop this snapshot from being restored.
    #[must_use]
    pub fn blocking(&self) -> Vec<&SessionResidency> {
        self.residencies
            .iter()
            .filter(|r| r.is_blocking())
            .collect()
    }

    /// Whether this build can read the document at all.
    #[must_use]
    pub fn is_readable(&self) -> bool {
        self.version == Self::VERSION
    }

    /// Rebuild the session on another process.
    ///
    /// Refused — never partially applied — when the document is from a
    /// format this build does not know, or when it carries a residency
    /// whose loss the restored session could not honestly survive. On
    /// success every severable residency comes back in
    /// [`RestoredSession::severed`], so what the move cost is on the
    /// record rather than absent from it.
    pub fn restore(&self) -> Result<RestoredSession, EngineError> {
        if !self.is_readable() {
            return Err(EngineError::Precondition(format!(
                "session snapshot for '{}' is format version {}, and this build reads only version {}",
                self.session_id,
                self.version,
                Self::VERSION
            )));
        }

        let blocking = self.blocking();
        if !blocking.is_empty() {
            let named = blocking
                .iter()
                .map(|r| r.describe())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(EngineError::Precondition(format!(
                "session '{}' cannot migrate while it holds {}",
                self.session_id, named
            )));
        }

        let status = self.status.migrates_as();
        let resumed_from = (status != self.status).then_some(self.status);
        let session = EngineSession::restored(
            &self.session_id,
            status,
            self.phase,
            self.cost.clone(),
            self.created_at,
            self.messages.clone(),
        );

        Ok(RestoredSession {
            session,
            iteration: self.iteration.clone(),
            consecutive_no_commits: self.consecutive_no_commits,
            roadmap: self.roadmap.clone(),
            roadmap_cursor: self.roadmap_cursor.clone(),
            severed: self.residencies.clone(),
            resumed_from,
        })
    }
}

#[cfg(test)]
#[path = "session_snapshot.tests.rs"]
mod tests;
