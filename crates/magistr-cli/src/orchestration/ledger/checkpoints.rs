//! The checkpoint spine as a service: save, restore, fork.

use std::sync::Arc;

use chrono::Utc;
use magistr_engine::{CheckpointReason, EngineHandle, RestoredSession};

use super::checkpoint::Checkpoint;
use super::checkpoint_spine::CheckpointSpine;
use super::repo::Repo;

/// The session's spine, over the engine that holds its state and the
/// repository that holds its tree.
///
/// The three verbs are the three things a ledger rail offers, and each
/// one is the join this module exists to make:
///
/// - [`save`] captures the engine with `EngineHandle::capture` and
///   the repo *in the same breath*, so the two halves of a checkpoint
///   describe the same instant rather than two instants that happen to
///   be close;
/// - [`restore`] hands the chosen snapshot to `EngineHandle::resume`,
///   which forces the phase and restores the iteration — the workflow
///   continues rather than starting over — and returns what the move
///   could not bring with it;
/// - [`fork`] opens a worktree branch at the checkpoint's commit, so
///   head continues untouched.
///
/// [`save`]: Checkpoints::save
/// [`restore`]: Checkpoints::restore
/// [`fork`]: Checkpoints::fork
pub struct Checkpoints {
    engine: EngineHandle,
    repo: Arc<dyn Repo>,
    spine: CheckpointSpine,
}

impl Checkpoints {
    /// The spine for `engine`, reading and forking `repo`.
    #[must_use]
    pub fn new(engine: EngineHandle, repo: Arc<dyn Repo>) -> Self {
        Self {
            engine,
            repo,
            spine: CheckpointSpine::new(),
        }
    }

    /// The chain as it stands.
    #[must_use]
    pub const fn spine(&self) -> &CheckpointSpine {
        &self.spine
    }

    /// Capture the session and repository under `id`, then append the checkpoint.
    ///
    /// Repository metadata is stored in the snapshot. The caller supplies the ID
    /// so a permission card can display it before saving; callers without a
    /// preassigned ID can use [`next_id`].
    ///
    /// [`next_id`]: Checkpoints::next_id
    pub fn save(
        &mut self,
        id: &str,
        reason: CheckpointReason,
        label: impl Into<String>,
    ) -> Result<&Checkpoint, String> {
        let mut snapshot = self.engine.capture();
        if let Some(state) = self.repo.state() {
            snapshot = snapshot.in_repo(state);
        }
        self.spine.save(id, reason, label, snapshot, Utc::now())
    }

    /// The next name to give a checkpoint, for a caller that is not
    /// numbering them itself.
    #[must_use]
    pub fn next_id(&self) -> String {
        self.spine.next_id()
    }

    /// Resume the session from the checkpoint `id` names.
    ///
    /// Refuses an unknown id, and passes the engine's own refusals
    /// through — a snapshot carrying a blocking residency is not
    /// restorable and `resume` says so rather than half-restoring it.
    pub fn restore(&self, id: &str) -> Result<RestoredSession, String> {
        let checkpoint = self
            .spine
            .get(id)
            .ok_or_else(|| format!("no checkpoint {id}"))?;
        self.engine
            .resume(&checkpoint.snapshot)
            .map_err(|e| e.to_string())
    }

    /// Open a worktree branch at the checkpoint `id` names, leaving
    /// head where it is. Returns the branch and the path it opened at.
    ///
    /// Refuses a checkpoint with no commit rather than forking from
    /// whatever head happens to be now: a fork that silently branched
    /// from somewhere other than the state the user picked would be the
    /// one failure this verb must not have.
    pub fn fork(&self, id: &str, branch: &str) -> Result<(String, String), String> {
        let checkpoint = self
            .spine
            .get(id)
            .ok_or_else(|| format!("no checkpoint {id}"))?;
        let head = checkpoint
            .head()
            .ok_or_else(|| format!("{id} was not taken in a git repository"))?;
        let path = self.repo.fork(branch, head)?;
        Ok((branch.to_owned(), path))
    }

    /// Bring the worktree forked at `id` home: merge the branch a fork
    /// of it opened, and answer that branch and the commit the merge
    /// landed on.
    ///
    /// The branch is *derived* from the checkpoint by the same rule
    /// [`default_branch`](Self::default_branch) forked it under, so a
    /// review approves the tree its own fork opened and there is no
    /// second name for the two to disagree through. A checkpoint the
    /// spine does not hold is refused before git is asked, which is
    /// where the useful error is.
    pub fn merge(&self, id: &str) -> Result<(String, String), String> {
        self.spine
            .get(id)
            .ok_or_else(|| format!("no checkpoint {id}"))?;
        let branch = Self::default_branch(id);
        let head = self.repo.merge(&branch)?;
        Ok((branch, head))
    }

    /// The branch a fork of `id` gets when the caller names none.
    #[must_use]
    pub fn default_branch(id: &str) -> String {
        format!("magistr/{id}")
    }
}

#[cfg(test)]
#[path = "checkpoints.tests.rs"]
mod tests;
