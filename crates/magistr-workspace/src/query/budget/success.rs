use crate::contracts::{
    Budget, EnvelopeVersion, ErrorKind, Freshness, ResultSet, TruncationReason, WorkspaceError,
    WorkspaceResponse,
};
use serde::Serialize;

pub(in crate::query) fn success<R: Serialize>(
    rows: Vec<R>,
    requested: u64,
    freshness: Freshness,
    notes: Vec<String>,
    cursor: impl Fn(&R) -> String,
    narrow: Vec<String>,
) -> WorkspaceResponse<R> {
    let mut used = 0;
    let mut keep = 0;
    for row in &rows {
        let cost = match super::tokens(row) {
            Ok(cost) => cost,
            Err(error) => return super::failure(error),
        };
        if used + cost > requested {
            break;
        }
        used += cost;
        keep += 1;
    }
    if keep == 0 && !rows.is_empty() {
        return super::failure(WorkspaceError::new(
            ErrorKind::BudgetTooSmall,
            "budget cannot fit the first result; increase budget or narrow the query",
        ));
    }
    let next = rows.get(keep).map(cursor).unwrap_or_default();
    let results = match ResultSet::truncate(rows, keep, TruncationReason::Budget, narrow, next) {
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
