//! One verified state on the spine.

use chrono::{DateTime, Utc};
use magistr_engine::{CheckpointReason, SessionSnapshot};

/// A restorable engine snapshot with its repository metadata, capture reason,
/// and the ID used by the rail and tool cards.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkpoint {
    /// `cp-1`, `cp-2`, … — minted by the spine, stable for the session.
    pub id: String,
    /// What put it here.
    pub reason: CheckpointReason,
    /// What was about to happen, in the words the reader will see —
    /// the tool a risk checkpoint precedes, the iteration a boundary
    /// closes.
    pub label: String,
    /// When it was taken.
    pub taken_at: DateTime<Utc>,
    /// The session as it stood.
    pub snapshot: SessionSnapshot,
}

impl Checkpoint {
    /// Whether the checkpoint's commit fully describes the tree it was
    /// taken from — false when the tree was dirty, and false when there
    /// was no repository to describe it at all.
    ///
    /// This is what "verified" means on a spine of verified states: a
    /// checkpoint a fork can actually reproduce.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.snapshot
            .repo
            .as_ref()
            .is_some_and(magistr_engine::RepoState::is_clean)
    }

    /// The commit a fork of this checkpoint would branch from.
    #[must_use]
    pub fn head(&self) -> Option<&str> {
        self.snapshot.repo.as_ref().map(|repo| repo.head.as_str())
    }

    /// The row the rail prints: `cp-3 · risk · rm -rf build`, with the
    /// short sha when there is one.
    #[must_use]
    pub fn row(&self) -> String {
        let mut row = format!("{} · {}", self.id, self.reason);
        if let Some(repo) = &self.snapshot.repo {
            row.push_str(&format!(" · {}", repo.short_head()));
        }
        if !self.label.is_empty() {
            row.push_str(&format!(" · {}", self.label));
        }
        row
    }
}
