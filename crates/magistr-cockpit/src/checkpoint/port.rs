//! Where a checkpoint is actually taken.

use magistr_engine::CheckpointReason;

/// The three verbs of the ledger rail, behind a port.
///
/// Each answers what the cockpit needs to *show*, not the state
/// itself — a save answers the commit and whether it is reproducible, a
/// restore answers where the workflow landed and what it could not
/// bring, a fork answers the branch and where it opened. The session
/// state stays on the other side, which is why the model can hold a
/// chain of two hundred checkpoints for the price of two hundred
/// strings.
///
/// `&mut self` because a spine grows; `Send + Sync` because the
/// consumer's task holds it across awaits, as [`SessionPolicy`] is.
///
/// [`SessionPolicy`]: crate::agent::SessionPolicy
pub trait CheckpointPort: Send + Sync {
    /// Take a checkpoint under `id`, and answer `(head, verified)`.
    fn save(
        &mut self,
        id: &str,
        reason: CheckpointReason,
        label: &str,
    ) -> Result<(Option<String>, bool), String>;

    /// Resume the session from `id`, and answer `(iteration, severed)`.
    fn restore(&mut self, id: &str) -> Result<(u32, Vec<String>), String>;

    /// Open a worktree branch at `id`, and answer `(branch, path)`.
    /// `branch` is `None` when the caller wants the port's default.
    fn fork(&mut self, id: &str, branch: Option<&str>) -> Result<(String, String), String>;

    /// Merge the worktree forked at `id` back, and answer
    /// `(branch, head)`.
    ///
    /// The fourth verb, and the inverse of [`fork`](Self::fork): a
    /// review's *Approve & merge* is what a forked worktree is *for*.
    /// A checkpoint that was never forked has no branch to bring home
    /// and the port says so in its own words — the refusal is the
    /// answer, not a silent success.
    fn merge(&mut self, id: &str) -> Result<(String, String), String>;
}
