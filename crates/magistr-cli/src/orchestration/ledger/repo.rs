//! The git working tree, as a port.

use magistr_engine::RepoState;

/// What the checkpoint spine needs from a repository, and no more.
///
/// Three verbs, because a checkpoint has three moments: reading the
/// state when it is taken, opening a branch at that state when it is
/// forked, and bringing that branch home when the work on it is
/// approved. All three are behind a port so the spine's own rules — what gets
/// appended, what a restore refuses, what a fork of a dirty checkpoint
/// means — are provable without a repository on disk, and so the
/// shelling out has exactly one home ([`GitRepo`](super::GitRepo)).
///
/// [`state`] answers `None` rather than an error for "this is not a git
/// repository": a session in a plain directory is a perfectly ordinary
/// session, and a checkpoint there simply carries no repo. A fork of
/// such a checkpoint is what fails, and it fails at the point where the
/// missing sha actually matters.
///
/// [`state`]: Repo::state
pub trait Repo: Send + Sync {
    /// The repository as it stands, or `None` when there is not one.
    fn state(&self) -> Option<RepoState>;

    /// Open a working tree at `head` on a new branch named `branch`,
    /// and return where it was opened.
    ///
    /// The existing tree is untouched — head continues. That is the
    /// difference between this and a checkout, and it is the whole
    /// reason a fork is a worktree.
    fn fork(&self, branch: &str, head: &str) -> Result<String, String>;

    /// Merge `branch` into the tree at the root, and return the commit
    /// the merge landed on.
    ///
    /// The inverse of [`fork`](Repo::fork): what a forked worktree is
    /// for is coming back. A branch that does not exist is a refusal in
    /// the repository's own words, which is the honest answer to
    /// "approve the work in a worktree nobody opened".
    fn merge(&self, branch: &str) -> Result<String, String>;
}
