use serde::{Deserialize, Serialize};

/// Repository metadata captured with a session.
///
/// The commit, branch, and dirty-file count identify the tree a snapshot was
/// produced against. The working tree itself is not included. Unlike a
/// [`SessionResidency`], this metadata can travel between machines.
///
/// The engine never reads Git; the adapter supplies this record.
///
/// [`SessionResidency`]: super::session_residency::SessionResidency
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoState {
    /// The commit `HEAD` pointed at — the full sha, so it is unambiguous
    /// wherever it is resolved.
    pub head: String,
    /// The branch `HEAD` was on, when it was on one. A detached head has
    /// none, and saying so is better than inventing `HEAD` as a name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// How many files the working tree had modified, added or deleted
    /// against `head`.
    ///
    /// A count and not a list: the point is whether the checkpoint's
    /// commit describes the tree it was taken from, which is a yes/no
    /// the number answers, and a list of paths in every snapshot would
    /// grow the document without making the answer any better.
    #[serde(default)]
    pub dirty: u32,
}

impl RepoState {
    /// The state of a repository at `head`, with nothing modified.
    #[must_use]
    pub fn at(head: impl Into<String>) -> Self {
        Self {
            head: head.into(),
            branch: None,
            dirty: 0,
        }
    }

    /// Record the branch `HEAD` was on.
    #[must_use]
    pub fn on_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Record how many files were dirty against `head`.
    #[must_use]
    pub const fn with_dirty(mut self, dirty: u32) -> Self {
        self.dirty = dirty;
        self
    }

    /// Whether `head` fully describes the tree this was captured from.
    ///
    /// A clean checkpoint can be reached again by anyone with the sha; a
    /// dirty one names a commit the tree had already moved past, and a
    /// caller that offers to restore or fork it should say so rather
    /// than promise a state it cannot rebuild.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.dirty == 0
    }

    /// The short form of `head` — the first seven characters, which is
    /// what git itself prints and what a rail column has room for.
    #[must_use]
    pub fn short_head(&self) -> &str {
        let end = self
            .head
            .char_indices()
            .nth(7)
            .map_or(self.head.len(), |(i, _)| i);
        &self.head[..end]
    }
}

#[cfg(test)]
#[path = "repo_state.tests.rs"]
mod tests;
