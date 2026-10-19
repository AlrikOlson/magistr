use std::sync::atomic::{AtomicU64, Ordering};

use super::WorkerError;

/// The next request id of one supervisor; ids are never reused across
/// incarnations, so a stale reply can never match a later request.
pub(super) fn next_request_id(next_id: &AtomicU64) -> Result<u64, WorkerError> {
    let mut current = next_id.load(Ordering::Relaxed);
    loop {
        let next = current
            .checked_add(1)
            .ok_or_else(|| WorkerError::Protocol("request ids exhausted".into()))?;
        match next_id.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(id) => return Ok(id),
            Err(observed) => current = observed,
        }
    }
}

#[cfg(test)]
#[path = "request_id.tests.rs"]
mod tests;
