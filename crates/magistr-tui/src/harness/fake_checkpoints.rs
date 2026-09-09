//! A scripted checkpoint port for the harness.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_engine::CheckpointReason;

use magistr_cockpit::checkpoint::CheckpointPort;

/// A [`CheckpointPort`] made of a script: every save is remembered and
/// answered with a scripted commit, a restore answers a scripted
/// iteration, a fork answers the branch it was given (or the port's
/// default) with a made-up path, and a merge answers the branch a fork
/// of that checkpoint would have opened. Clones share the log, so a test
/// keeps one and boxes the other — the witness that a risky permission
/// really asked for a checkpoint, and that `/restore` and `/fork` on
/// the ledger really reached the spine.
#[derive(Clone, Debug, Default)]
pub struct FakeCheckpoints {
    saved: Arc<Mutex<Vec<(String, CheckpointReason, String)>>>,
    restored: Arc<Mutex<Vec<String>>>,
    forked: Arc<Mutex<Vec<(String, String)>>>,
    merged: Arc<Mutex<Vec<String>>>,
    head: Option<String>,
    verified: bool,
    iteration: u32,
    severed: Vec<String>,
    refuse: Option<String>,
}

impl FakeCheckpoints {
    /// A port that takes every checkpoint, in no repository — so a
    /// checkpoint is saved and unverified, which is what a session
    /// outside git really gets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The port capturing in a repository at `head`, with a clean tree.
    #[must_use]
    pub fn in_repo(mut self, head: impl Into<String>) -> Self {
        self.head = Some(head.into());
        self.verified = true;
        self
    }

    /// The port restoring to `iteration`, leaving `severed` behind.
    #[must_use]
    pub fn restoring_to(mut self, iteration: u32, severed: &[&str]) -> Self {
        self.iteration = iteration;
        self.severed = severed.iter().map(|s| (*s).to_owned()).collect();
        self
    }

    /// The port refusing everything with `why`.
    #[must_use]
    pub fn refusing(mut self, why: impl Into<String>) -> Self {
        self.refuse = Some(why.into());
        self
    }

    /// Every checkpoint asked for, in order.
    #[must_use]
    pub fn saved(&self) -> Vec<(String, CheckpointReason, String)> {
        lock(&self.saved).clone()
    }

    /// Every checkpoint restored from, in order.
    #[must_use]
    pub fn restored(&self) -> Vec<String> {
        lock(&self.restored).clone()
    }

    /// Every fork, as `(checkpoint, branch)`.
    #[must_use]
    pub fn forked(&self) -> Vec<(String, String)> {
        lock(&self.forked).clone()
    }

    /// Every checkpoint whose worktree was merged home, in order.
    #[must_use]
    pub fn merged(&self) -> Vec<String> {
        lock(&self.merged).clone()
    }
}

impl CheckpointPort for FakeCheckpoints {
    fn save(
        &mut self,
        id: &str,
        reason: CheckpointReason,
        label: &str,
    ) -> Result<(Option<String>, bool), String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        lock(&self.saved).push((id.to_owned(), reason, label.to_owned()));
        Ok((self.head.clone(), self.verified))
    }

    fn restore(&mut self, id: &str) -> Result<(u32, Vec<String>), String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        lock(&self.restored).push(id.to_owned());
        Ok((self.iteration, self.severed.clone()))
    }

    fn fork(&mut self, id: &str, branch: Option<&str>) -> Result<(String, String), String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        let branch = branch.map_or_else(|| format!("magistr/{id}"), str::to_owned);
        lock(&self.forked).push((id.to_owned(), branch.clone()));
        Ok((branch.clone(), format!("/tmp/{branch}")))
    }

    fn merge(&mut self, id: &str) -> Result<(String, String), String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        lock(&self.merged).push(id.to_owned());
        Ok((format!("magistr/{id}"), "merge01".to_owned()))
    }
}

fn lock<T>(cell: &Mutex<T>) -> MutexGuard<'_, T> {
    cell.lock().unwrap_or_else(PoisonError::into_inner)
}
