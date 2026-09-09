//! What the checkpoint spine answered.

/// The outcome of a [`CheckpointCmd`](super::CheckpointCmd), posted on
/// the inbox as [`Msg::Checkpoint`](super::Msg::Checkpoint) and folded
/// into the rail's spine and the stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointEvent {
    /// The state was taken.
    Saved {
        /// The name it was taken under — the row it lands on.
        id: String,
        /// The commit a fork of it would branch from, when it was taken
        /// in a repository.
        head: Option<String>,
        /// Whether that commit describes the tree.
        verified: bool,
    },
    /// The session resumed from a checkpoint.
    Restored {
        /// The state it went back to.
        id: String,
        /// The iteration the workflow is at again.
        iteration: u32,
        /// What the resume could not bring with it, named rather than
        /// dropped — the engine's own honesty, carried to the stage.
        severed: Vec<String>,
    },
    /// A worktree branch was opened at a checkpoint; head continues.
    Forked {
        /// The state it branched from.
        id: String,
        /// The branch that was opened.
        branch: String,
        /// Where the working tree was opened.
        path: String,
    },
    /// A forked worktree was merged back into the session's tree.
    Merged {
        /// The state it was forked at.
        id: String,
        /// The branch that came home.
        branch: String,
        /// The commit the merge landed on.
        head: String,
    },
    /// The port refused.
    Failed {
        /// The checkpoint the request named, when it named one — so a
        /// failed save can mark its own row rather than only leaving a
        /// line on the stage.
        id: Option<String>,
        /// What was asked (`save cp-4`, `restore cp-2`, `fork cp-2`,
        /// `merge cp-3`).
        what: String,
        /// The port's reason.
        error: String,
    },
}
