//! Write a finished session to the cost ledger at its default path.

use std::path::{Path, PathBuf};

use super::costs_path::costs_path;
use super::record_session_at::record_session_at;
use super::session_spend::SessionSpend;

/// Write `spend` to this machine's cost ledger for a run in `dir`.
///
/// Returns the path written, or `None` when there was nothing to record
/// or nowhere to record it. Callers ignore the result: see
/// [`record_session_at`] for why a failed write is not an error.
pub fn record_session(dir: &Path, spend: &SessionSpend) -> Option<PathBuf> {
    let ledger = costs_path()?;
    record_session_at(&ledger, dir, spend).then_some(ledger)
}
