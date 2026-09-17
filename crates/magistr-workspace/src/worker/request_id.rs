use std::sync::atomic::{AtomicU64, Ordering};

use super::WorkerError;

/// The next request id of one supervisor; ids are never reused across
/// incarnations, so a stale reply can never match a later request.
pub(super) fn next_request_id(next_id: &AtomicU64) -> Result<u64, WorkerError> {
    next_id
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| WorkerError::Protocol("request ids exhausted".into()))
}
