use crate::contracts::{
    Budget, EnvelopeVersion, Freshness, ResultSet, TruncationReason, WorkspaceResponse,
};
use serde::Serialize;

/// The answer of a settled mutation. Unlike a query answer, no row is ever
/// cut and the budget never refuses: the write already happened (or, on a dry
/// run, provably did not), so every row's outcome must reach the caller. Rows
/// that overflow the budget are compacted by `compact` (true when it dropped
/// detail), and the manifest plus a note declare what was omitted.
pub(crate) fn receipt<R: Serialize>(
    mut rows: Vec<R>,
    requested: u64,
    freshness: Freshness,
    mut notes: Vec<String>,
    compact: impl Fn(&mut R) -> bool,
    cursor: impl Fn(&R) -> String,
) -> WorkspaceResponse<R> {
    let mut used = 0;
    let mut compacted = 0;
    let mut first = String::new();
    for row in &mut rows {
        let mut cost = match super::tokens(row) {
            Ok(cost) => cost,
            Err(error) => return super::failure(error),
        };
        if used + cost > requested && compact(row) {
            if compacted == 0 {
                first = cursor(row);
            }
            compacted += 1;
            cost = match super::tokens(row) {
                Ok(cost) => cost,
                Err(error) => return super::failure(error),
            };
        }
        used += cost;
    }
    if compacted > 0 {
        notes.push(format!(
            "budget {requested} cannot fit the full receipt: {compacted} diff(s) omitted. Each row's applied/disposition is the settled outcome — do not re-issue the call to see the diff; read the path (contentHash names the final bytes) or list the journal seq"
        ));
    }
    let results = match ResultSet::with_manifest(
        rows,
        compacted,
        TruncationReason::Budget,
        vec!["raise budget".into(), "read the path".into()],
        first,
    ) {
        Ok(results) => results,
        Err(error) => return super::failure(error),
    };
    WorkspaceResponse::Success {
        version: EnvelopeVersion::LATEST,
        freshness,
        budget: Budget { requested, used },
        results,
        notes: (!notes.is_empty()).then_some(notes),
    }
}

#[cfg(test)]
#[path = "receipt.tests.rs"]
mod tests;
