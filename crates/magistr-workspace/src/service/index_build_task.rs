use super::IndexBuildState;
use crate::{contracts::Freshness, index::IndexStatus};

/// The `index/build` task, derived purely from the index status — there is
/// no task store, so any process over the same workspace derives the same
/// answer and a process whose index never built has no task. A build in
/// flight is working; a recorded failure is failed; a warm index after a
/// build is completed; a kicked, unfinished, error-free cold index is the
/// settle race between the last commit and the status flip, and working is
/// the only honest label until the client polls again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexBuildTask {
    pub state: IndexBuildState,
    pub message: String,
    /// Milliseconds since the Unix epoch when the build started.
    pub created_at_ms: u64,
    /// Milliseconds since the Unix epoch of the last status change.
    pub last_updated_at_ms: u64,
    /// Present while working: the retained fast cadence, because builds are.
    pub poll_interval_ms: Option<u64>,
}

impl IndexBuildTask {
    pub const ID: &'static str = "index/build";
    pub const POLL_INTERVAL_MS: u64 = 200;

    #[must_use]
    pub fn derive(status: &IndexStatus) -> Option<Self> {
        let created_at_ms = status.build_started_at_ms?;
        let last_updated_at_ms = status.build_updated_at_ms.unwrap_or(created_at_ms);
        let indexed = status.parsed + status.reused;
        let total = status.files;
        let working = || Self {
            state: IndexBuildState::Working,
            message: format!("index build in progress: {indexed}/{total} files"),
            created_at_ms,
            last_updated_at_ms,
            poll_interval_ms: Some(Self::POLL_INTERVAL_MS),
        };
        if status.building {
            return Some(working());
        }
        if let Some(error) = &status.last_error {
            return Some(Self {
                state: IndexBuildState::Failed,
                message: format!("index build failed: {error}"),
                created_at_ms,
                last_updated_at_ms,
                poll_interval_ms: None,
            });
        }
        if status.refresh_pending {
            return Some(working());
        }
        if status.freshness != Freshness::Cold {
            return Some(Self {
                state: IndexBuildState::Completed,
                message: format!(
                    "index at tier 1, generation {} ({total} files)",
                    status.generation
                ),
                created_at_ms,
                last_updated_at_ms,
                poll_interval_ms: None,
            });
        }
        Some(working())
    }

    /// The task of an index whose first build was kicked at `opened_at_ms`:
    /// until the coordinator stamps its own start time the build is
    /// honestly working since open (or failed, if opening the cache failed),
    /// so a client that asks a moment after `initialize` is never told the
    /// task does not exist.
    #[must_use]
    pub fn since_open(status: &IndexStatus, opened_at_ms: u64) -> Option<Self> {
        if status.build_started_at_ms.is_some() {
            return Self::derive(status);
        }
        let mut status = status.clone();
        status.build_started_at_ms = Some(opened_at_ms);
        status.build_updated_at_ms = Some(opened_at_ms);
        Self::derive(&status)
    }
}

#[cfg(test)]
#[path = "index_build_task.tests.rs"]
mod tests;
