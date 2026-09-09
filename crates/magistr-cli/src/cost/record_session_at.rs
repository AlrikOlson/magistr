//! The production writer for the cost ledger, against an explicit path.
//!
//! Before this existed, `CostEntry` had exactly one caller — the
//! integration tests — so every windowed budget scope resolved against
//! an empty file and a `daily` or `monthly` cap could never actually
//! bind. A governance hierarchy that nothing feeds is arithmetic, not
//! enforcement. This closes that loop.
//!
//! Every failure here is swallowed on purpose. Book-keeping must never
//! be the reason a finished session reports an error to the user: the
//! work is already done, and the worst outcome of a missed write is a
//! slightly generous cap next window.

use std::path::Path;

use chrono::Utc;

use super::append_entry::append_entry;
use super::cost_entry::CostEntry;
use super::session_spend::SessionSpend;
use crate::team::Identity;

/// Write `spend` to the ledger at `ledger` for a run in `dir`, stamping
/// the resolved identity as the actor and the run's model.
///
/// Returns whether an entry was written.
pub fn record_session_at(ledger: &Path, dir: &Path, spend: &SessionSpend) -> bool {
    if spend.is_empty() {
        return false;
    }
    let entry = cost_entry(dir, spend);
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
fn cost_entry(dir: &Path, spend: &SessionSpend) -> CostEntry {
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
        actor: Some(Identity::resolve(dir).id().to_owned()),
        model: spend.model.clone(),
    }
}

#[cfg(test)]
#[path = "record_session_at.tests.rs"]
mod tests;
