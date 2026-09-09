use serde::{Deserialize, Serialize};

/// One project's place in a batch: what it is called, what must finish
/// before it starts, and which working tree it writes into.
///
/// `deps` names other projects in the same batch. An edge to a project
/// the batch does not carry is not an error and not a wait — a batch is
/// a selection, and a dependency outside the selection is one the
/// operator has decided is not part of this run. [`ProjectGraph::order`]
/// therefore orders on the edges it can see and never blocks on an edge
/// it cannot.
///
/// `worktree` is the monorepo half, and it is a *restriction a project
/// opts into* rather than a fact every project must supply — the same
/// reading [`FleetNode::projects`] has, where an empty list is
/// general-purpose rather than useless. A project with no declared tree
/// is its own checkout and shares with nobody; two projects naming the
/// same tree are two archetypes of one monorepo, and the batch will not
/// run them at the same time. That is not politeness about a lock: git
/// is a single-writer store, and two agents running `add`/`commit`/
/// `checkout` against one index race on `.git/index.lock`. The
/// ecosystem's answer is retry-with-backoff; a scheduler that already
/// holds the graph does not need to guess.
///
/// [`ProjectGraph::order`]: super::project_graph::ProjectGraph::order
/// [`FleetNode::projects`]: crate::fleet::FleetNode::projects
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDep {
    /// The project id, as the project registry knows it.
    pub id: String,
    /// The projects that must succeed before this one is released.
    #[serde(default)]
    pub deps: Vec<String>,
    /// The working tree this project writes into, when it shares one.
    #[serde(default)]
    pub worktree: Option<String>,
}

impl ProjectDep {
    /// A project with no dependencies and a checkout of its own.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            deps: Vec::new(),
            worktree: None,
        }
    }

    /// Declare what must finish first (self-consuming builder).
    #[must_use]
    pub fn after<I, S>(mut self, deps: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.deps = deps.into_iter().map(Into::into).collect();
        self
    }

    /// Declare the working tree this project shares (self-consuming
    /// builder).
    #[must_use]
    pub fn in_worktree(mut self, worktree: impl Into<String>) -> Self {
        self.worktree = Some(worktree.into());
        self
    }

    /// Whether this project and `other` write into the same tree, and so
    /// must never run at the same time.
    ///
    /// A project with no declared tree shares with nobody — including
    /// another project with no declared tree, which is a *different*
    /// unnamed checkout rather than the same one.
    #[must_use]
    pub fn shares_worktree_with(&self, other: &Self) -> bool {
        match (&self.worktree, &other.worktree) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "project_dep.tests.rs"]
mod tests;
