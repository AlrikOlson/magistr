//! The session settlement adapter for the existing cost ledger.
//! It stamps captured project attribution and stores one cumulative conversation
//! entry. A write failure is reported separately and cannot change the outcome
//! the provider actually returned.

use std::path::Path;

use chrono::Utc;

use super::append_entry::append_entry;
use super::cost_entry::CostEntry;
use super::session_spend::SessionSpend;
use crate::config::inputs::ProjectInputs;
use crate::team::Identity;

/// Write `spend` to the ledger at `ledger` for a run in `dir`, stamping
/// the identity resolved from `dir` and the captured `inputs` as the
/// actor, and the run's model.
///
/// Returns whether an entry was written.
pub fn record_session_at(
    ledger: &Path,
    dir: &Path,
    inputs: &ProjectInputs,
    spend: &SessionSpend,
) -> bool {
    if spend.is_empty() {
        return false;
    }
    let entry = cost_entry(dir, inputs, spend);
    match append_entry(ledger, &entry) {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(%err, ledger = %ledger.display(), "could not record session cost");
            false
        }
    }
}

/// Build the ledger row for a session, resolving the actor and project
/// the caller does not carry.
fn cost_entry(dir: &Path, inputs: &ProjectInputs, spend: &SessionSpend) -> CostEntry {
    CostEntry {
        session_id: spend.session_id.clone(),
        project: dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        timestamp: Utc::now(),
        cost_usd: spend.cost_usd,
        tokens: spend.tokens.clone(),
        turns: spend.turns,
        duration_ms: spend.duration_ms,
        actor: Some(Identity::resolve(dir, inputs).id().to_owned()),
        model: spend.model.clone(),
    }
}

#[cfg(test)]
#[path = "record_session_at.tests.rs"]
mod tests;
