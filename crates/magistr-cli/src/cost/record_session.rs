//! Write a finished session to the cost ledger at its default path.

use std::path::{Path, PathBuf};

use super::costs_path::costs_path;
use super::record_session_at::record_session_at;
use super::session_spend::SessionSpend;
use crate::config::inputs::ProjectInputs;

/// Write `spend` to this machine's cost ledger for a run in `dir`, the
/// ledger and the actor both resolved from the captured `inputs`.
///
/// Returns the path written, or `None` when there was nothing to record
/// or nowhere to record it. Callers ignore the result: see
/// [`record_session_at`] for why a failed write is not an error.
pub fn record_session(inputs: &ProjectInputs, dir: &Path, spend: &SessionSpend) -> Option<PathBuf> {
    let ledger = costs_path(inputs)?;
    record_session_at(&ledger, dir, inputs, spend).then_some(ledger)
}
