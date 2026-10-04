//! One assessment for scheduling, admission and projected roadmap readers.
use super::{Chunk, ChunkStatus};

/// An unmet dependency and its status in the same complete snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingDependency<'a> {
    pub id: &'a str,
    /// None means the required record is absent.
    pub status: Option<ChunkStatus>,
}

/// Derived eligibility, never persisted alongside the canonical record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness<'a> {
    Ready,
    Blocked(&'a str),
    Waiting(Vec<WaitingDependency<'a>>),
    Inactive(ChunkStatus),
}

impl Readiness<'_> {
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// Assess a record against its complete snapshot. A missing dependency fails
/// closed; only Done satisfies one. A recorded blocker takes precedence over
/// dependency reasons for unfinished work. Terminal records remain inactive,
/// and lifecycle status never implies readiness by itself.
#[must_use]
pub fn assess_readiness<'a>(
    status: ChunkStatus,
    blocker: Option<&'a str>,
    deps: &'a [String],
    mut dependency_status: impl FnMut(&str) -> Option<ChunkStatus>,
) -> Readiness<'a> {
    if matches!(status, ChunkStatus::Done | ChunkStatus::Obsoleted) {
        return Readiness::Inactive(status);
    }
    if let Some(reason) = blocker {
        return Readiness::Blocked(reason);
    }
    if status != ChunkStatus::Pending {
        return Readiness::Inactive(status);
    }
    let waiting: Vec<_> = deps
        .iter()
        .filter_map(|id| {
            let status = dependency_status(id);
            (status != Some(ChunkStatus::Done)).then_some(WaitingDependency { id, status })
        })
        .collect();
    if waiting.is_empty() {
        Readiness::Ready
    } else {
        Readiness::Waiting(waiting)
    }
}

impl Chunk {
    #[must_use]
    pub fn readiness(
        &self,
        dependency_status: impl FnMut(&str) -> Option<ChunkStatus>,
    ) -> Readiness<'_> {
        assess_readiness(
            self.status,
            self.blocked_by.as_ref().map(|b| b.reason.as_str()),
            &self.deps,
            dependency_status,
        )
    }
}

/// Lowest priority among eligible records; the first equal-priority record
/// wins. Callers preserve the canonical snapshot's insertion order.
#[must_use]
pub fn next_ready<'a, T: 'a>(
    items: impl IntoIterator<Item = &'a T>,
    mut eligible: impl FnMut(&T) -> bool,
    mut priority: impl FnMut(&T) -> u32,
) -> Option<&'a T> {
    items
        .into_iter()
        .filter(|item| eligible(item))
        .min_by_key(|item| priority(item))
}

#[cfg(test)]
#[path = "readiness.tests.rs"]
mod tests;
