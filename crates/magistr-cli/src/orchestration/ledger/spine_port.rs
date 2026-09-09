//! The cockpit's checkpoint port, over the real spine.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_engine::{CheckpointReason, EngineHandle};

use super::checkpoints::Checkpoints;
use super::git_repo::GitRepo;
use super::repo::Repo;

/// [`Checkpoints`] as the cockpit's [`CheckpointPort`].
///
/// The adapter is thin on purpose: the four verbs are already the four
/// the port names, and all this does is answer the *summaries* the rail
/// shows — the commit and whether it is reproducible, the iteration a
/// restore landed on and what it left behind, the branch a fork opened,
/// the commit a merge landed on — rather than handing the session state
/// itself up to a model that has no use for it.
///
/// The mutex is because the port is `Send + Sync` (the session task
/// holds it across awaits) while the spine is plainly mutable; there is
/// exactly one holder, so it is never contended.
pub struct SpinePort {
    checkpoints: Mutex<Checkpoints>,
}

impl SpinePort {
    /// The port over `engine`, checkpointing the repository at `root`.
    #[must_use]
    pub fn new(engine: EngineHandle, repo: Arc<dyn Repo>) -> Self {
        Self {
            checkpoints: Mutex::new(Checkpoints::new(engine, repo)),
        }
    }

    /// The port for a session running in `root`, over the project's git
    /// working tree.
    #[must_use]
    pub fn at(engine: EngineHandle, root: impl Into<std::path::PathBuf>) -> Self {
        Self::new(engine, Arc::new(GitRepo::at(root)))
    }

    fn spine(&self) -> MutexGuard<'_, Checkpoints> {
        self.checkpoints
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl CheckpointPort for SpinePort {
    fn save(
        &mut self,
        id: &str,
        reason: CheckpointReason,
        label: &str,
    ) -> Result<(Option<String>, bool), String> {
        let mut spine = self.spine();
        let checkpoint = spine.save(id, reason, label)?;
        Ok((
            checkpoint.head().map(str::to_owned),
            checkpoint.is_verified(),
        ))
    }

    fn restore(&mut self, id: &str) -> Result<(u32, Vec<String>), String> {
        let restored = self.spine().restore(id)?;
        Ok((
            restored.iteration.iteration,
            restored
                .severed
                .iter()
                .map(magistr_engine::SessionResidency::describe)
                .collect(),
        ))
    }

    fn fork(&mut self, id: &str, branch: Option<&str>) -> Result<(String, String), String> {
        let spine = self.spine();
        let branch = branch.map_or_else(|| Checkpoints::default_branch(id), str::to_owned);
        spine.fork(id, &branch)
    }

    fn merge(&mut self, id: &str) -> Result<(String, String), String> {
        self.spine().merge(id)
    }
}
