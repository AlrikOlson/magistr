//! The ledger: the session's chain of verified states.
//!
//! A checkpoint is two things joined — the engine's own
//! [`SessionSnapshot`] (status, phase, iteration, cost, the
//! roadmap cursor, the message log) and the [`RepoState`] the tree was
//! in when it was taken. Neither half is enough on its own: the
//! snapshot restores the *workflow* and says nothing about which commit
//! it was reading, and a sha restores the *tree* and says nothing about
//! which iteration produced it.
//!
//! [`Checkpoints`] is the service the cockpit's checkpoint port sits
//! on. It holds a [`CheckpointSpine`] — the chain, oldest first, ids
//! minted `cp-1`, `cp-2`, … — and answers three verbs:
//!
//! - **save** captures the engine and the repo together and appends;
//! - **restore** hands the chosen snapshot back through
//!   [`EngineHandle::resume`], so the workflow continues from that
//!   state rather than restarting;
//! - **fork** opens a git worktree branch at the checkpoint's commit,
//!   which leaves head exactly where it was — that is the whole point
//!   of a worktree rather than a `checkout`.
//!
//! git is behind the [`Repo`] port, so the spine's rules are testable
//! without a repository and the shelling out has one home
//! ([`GitRepo`]).
//!
//! [`SessionSnapshot`]: magistr_engine::SessionSnapshot
//! [`RepoState`]: magistr_engine::RepoState
//! [`EngineHandle::resume`]: magistr_engine::EngineHandle::resume

pub mod checkpoint;
pub mod checkpoint_spine;
pub mod checkpoints;
pub mod git_repo;
pub mod repo;
pub mod session_engine;
pub mod spine_port;

pub use checkpoint::Checkpoint;
pub use checkpoint_spine::CheckpointSpine;
pub use checkpoints::Checkpoints;
pub use git_repo::GitRepo;
pub use repo::Repo;
pub use session_engine::session_engine;
pub use spine_port::SpinePort;
