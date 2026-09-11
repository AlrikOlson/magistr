use super::JournalListInput;
use crate::{
    commands::UndoScope,
    contracts::{
        Budget, EnvelopeVersion, ErrorKind, Freshness, ResultSet, TruncationReason, WorkspaceError,
        WorkspaceResponse,
    },
    diagnostics::{JournalRow, JournalViewOptions},
    journal::{JournalDb, JournalError, JournalLogQuery},
    query::budget,
};

/// The retained wire estimator: JavaScript string length over four.
const CHARS_PER_TOKEN: usize = 4;

/// The journal as rows over the sole writer's own connection. One op beyond
/// the limit is fetched so "there is more" is a measured fact and the cursor
/// it produces is real; rows are cut on op boundaries to the token budget,
/// never to zero (one over-budget op beats a silent empty log); the manifest
/// names a `beforeSeq` the next call can pass straight back.
pub(crate) fn list(
    journal: &JournalDb,
    input: &JournalListInput,
    session_id: &str,
) -> WorkspaceResponse<JournalRow> {
    if let Err(error) = validate(input) {
        return budget::failure(error);
    }
    let mut notes = Vec::new();
    let scope = input.scope.as_str();
    if input.scope == UndoScope::Session && session_id.is_empty() {
        notes.push("undo: no session identity was supplied — session scope lists only ops journaled anonymously; pass a session id or scope: \"workspace\"".into());
    }
    let query = JournalLogQuery {
        session_id: (input.scope == UndoScope::Session).then(|| session_id.to_owned()),
        limit: input.limit + 1,
        since_seq: input.since_seq,
        before_seq: input.before_seq,
    };
    let entries = match journal.recent(&query) {
        Ok(entries) => entries,
        Err(error) => return budget::failure(map(error)),
    };
    let kept: Vec<i64> = entries
        .iter()
        .take(input.limit)
        .map(|entry| entry.op.seq)
        .collect();
    if kept.is_empty() {
        notes.push(format!("undo: the journal has no ops in {scope} scope"));
        return finish(input.budget, ResultSet::complete(Vec::new()), notes);
    }
    // Rendered over everything fetched, including the probe op, so what the
    // cut reports as dropped is a count of real rows, not an estimate.
    let rows = JournalRow::render(
        &entries,
        &JournalViewOptions {
            diff: input.diff,
            diff_char_cap: JournalViewOptions::DIFF_CHARS_MAX,
        },
    );
    let within_limit = rows.iter().filter(|row| kept.contains(&row.seq)).count();
    let keep = fit(&rows, within_limit, input.budget);
    let over_budget = keep < within_limit;
    let last_seq = keep
        .checked_sub(1)
        .and_then(|index| rows.get(index))
        .map_or_else(|| kept.last().copied().unwrap_or(0), |row| row.seq);
    let total = rows.len();
    let results = match ResultSet::truncate(
        rows,
        keep,
        if over_budget {
            TruncationReason::Budget
        } else {
            TruncationReason::Ceiling
        },
        vec![
            "limit".into(),
            "diff:false".into(),
            format!("beforeSeq:{last_seq}"),
        ],
        format!("beforeSeq:{last_seq}"),
    ) {
        Ok(results) => results,
        Err(error) => return budget::failure(error),
    };
    if over_budget {
        notes.push(format!(
            "undo: the listing was cut to fit the {}-token budget — continue with beforeSeq: {last_seq}, or pass diff: false for a leaner log",
            input.budget
        ));
    } else if keep < total {
        notes.push(format!(
            "undo: older ops remain — continue with beforeSeq: {last_seq}"
        ));
    }
    finish(input.budget, results, notes)
}

/// How many leading rows fit the budget, cut on an op boundary: the first
/// row over the line ends the listing before its op, unless that op is the
/// first one, which is kept whole.
fn fit(rows: &[JournalRow], within_limit: usize, budget: u64) -> usize {
    let char_budget = (budget as usize).saturating_mul(CHARS_PER_TOKEN);
    let mut chars = 0;
    for (index, row) in rows.iter().take(within_limit).enumerate() {
        chars += utf16_len(row) + 1;
        if chars > char_budget {
            let seq = row.seq;
            let op_start = rows.iter().position(|row| row.seq == seq).unwrap_or(0);
            return if op_start == 0 {
                rows.iter().filter(|row| row.seq == seq).count()
            } else {
                op_start.min(index.max(op_start))
            };
        }
    }
    within_limit
}

fn utf16_len(row: &JournalRow) -> usize {
    serde_json::to_string(row).map_or(0, |text| text.encode_utf16().count())
}

fn finish(
    requested: u64,
    results: ResultSet<JournalRow>,
    notes: Vec<String>,
) -> WorkspaceResponse<JournalRow> {
    let used = serde_json::to_string(results.rows()).map_or(1, |text| {
        text.encode_utf16().count().div_ceil(CHARS_PER_TOKEN)
    }) as u64;
    WorkspaceResponse::Success {
        version: EnvelopeVersion::LATEST,
        freshness: Freshness::Warm,
        budget: Budget { requested, used },
        results,
        notes: (!notes.is_empty()).then_some(notes),
    }
}

fn validate(input: &JournalListInput) -> Result<(), WorkspaceError> {
    let invalid = |message: String| Err(WorkspaceError::new(ErrorKind::PatternInvalid, message));
    if input.limit < 1 || input.limit > JournalListInput::LIMIT_MAX {
        return invalid(format!(
            "undo: limit must be an integer in [1, {}]",
            JournalListInput::LIMIT_MAX
        ));
    }
    for (name, value) in [
        ("sinceSeq", input.since_seq),
        ("beforeSeq", input.before_seq),
    ] {
        if value.is_some_and(|value| value < 0) {
            return invalid(format!(
                "undo: {name} must be a non-negative integer journal seq"
            ));
        }
    }
    if input.budget == 0 {
        return Err(WorkspaceError::new(
            ErrorKind::BudgetTooSmall,
            "undo: budget must be a positive integer",
        ));
    }
    Ok(())
}

fn map(error: JournalError) -> WorkspaceError {
    match error {
        JournalError::UnsupportedVersion(detail) => WorkspaceError::new(
            ErrorKind::UnsupportedVersion,
            format!(
                "the journal cannot be read by this binary: {detail}; upgrade magistr — durable data was left untouched"
            ),
        ),
        JournalError::InvalidInput(detail) => WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!("journal query: {detail}"),
        ),
        other => WorkspaceError::new(
            ErrorKind::EngineUnavailable,
            format!("the journal cannot be read: {other}"),
        ),
    }
}

#[cfg(test)]
#[path = "journal_list.tests.rs"]
mod tests;
