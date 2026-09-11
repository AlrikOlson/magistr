use std::collections::BTreeMap;

use super::{UndoInput, UndoRow, UndoScope, WorkspaceCommands, WriteReport};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    file_content::FileContent,
    journal::{JournalOp, UndoCandidateQuery},
    query::budget,
    transaction::{
        ExpectedState, FormatMode, TransactionChange, TransactionRequest, TransactionStatus,
        WorkspaceTransactions,
    },
    undo::UndoPlan,
};

const STEPS_MAX: usize = 100;

/// Session-scoped rewind with surfaced conflicts. Every restoration is a
/// pipeline batch with formatting disabled (exact journaled bytes) and
/// expected anchors, so a concurrent mutation fails closed. The scope
/// defaults to the calling session: two agents sharing a workspace never
/// silently revert each other. Restorations are never confirm-gated.
pub(super) async fn run(
    service: &mut WorkspaceCommands<'_>,
    input: &UndoInput,
) -> WorkspaceResponse<UndoRow> {
    if let Err(error) = validate(input) {
        return budget::failure(error);
    }
    let mut notes = Vec::new();
    if input.scope == UndoScope::Session && service.session_id.is_empty() {
        notes.push("undo: no session identity was supplied — session scope matches only ops journaled anonymously; pass a session id or scope: \"workspace\"".into());
    }
    if input.ops.is_some() && input.steps != 1 {
        notes.push("undo: steps is ignored when ops names the ops to rewind".into());
    }
    let candidates = match candidates(service, input, &mut notes) {
        Ok(Some(candidates)) => candidates,
        Ok(None) => return finish(input.budget, Vec::new(), notes),
        Err(error) => return budget::failure(error),
    };
    let mut current: BTreeMap<String, Option<FileContent>> = BTreeMap::new();
    for file in candidates.iter().flat_map(|op| &op.files) {
        if current.contains_key(&file.path) {
            continue;
        }
        match service.reads.read(&file.path) {
            Ok(content) => {
                current.insert(file.path.clone(), content);
            }
            Err(error) => return budget::failure(error),
        }
    }
    let plan = match UndoPlan::build(&candidates, &current, input.on_conflict) {
        Ok(plan) => plan,
        Err(error) => {
            return budget::failure(WorkspaceError::new(
                ErrorKind::PatternInvalid,
                error.to_string(),
            ));
        }
    };
    notes.extend(plan.notes.iter().cloned());
    let skipped: Vec<UndoRow> = plan
        .kept
        .iter()
        .map(|kept| UndoRow::Kept {
            path: kept.path.clone(),
            op_seq: kept.op_seq,
        })
        .chain(plan.evicted.iter().map(|evicted| UndoRow::Evicted {
            path: evicted.path.clone(),
            op_seq: evicted.op_seq,
        }))
        .collect();
    if !plan.conflicts.is_empty() {
        let mut rows: Vec<UndoRow> = plan
            .conflicts
            .iter()
            .map(|conflict| UndoRow::Conflict {
                path: conflict.path.clone(),
                op_seq: conflict.op_seq,
                expected_hash: conflict.expected_hash.clone().unwrap_or_default(),
                current_hash: conflict.current_hash.clone().unwrap_or_default(),
                restore_hash: conflict.restore_hash.clone().unwrap_or_default(),
            })
            .collect();
        notes.push("undo: conflicts detected — a later change touched these files; nothing was applied. Three ways forward: re-run with onConflict: \"keepCurrent\" to leave the conflicted files as they are, onConflict: \"force\" to restore the journaled versions anyway, or read the file and merge by hand using the hashes in each conflict row".into());
        rows.extend(
            skipped
                .into_iter()
                .filter(|row| matches!(row, UndoRow::Evicted { .. })),
        );
        return finish(input.budget, rows, notes);
    }
    let changes: Vec<TransactionChange> = plan
        .writes
        .iter()
        .map(|write| TransactionChange {
            path: write.path.clone(),
            expected: if write.expected_exists {
                ExpectedState::Hash(write.expected_hash.clone())
            } else {
                ExpectedState::Missing
            },
            content: write.content.clone(),
            baseline: None,
        })
        .collect();
    if changes.is_empty() && plan.undo_of.is_empty() {
        return finish(input.budget, skipped, notes);
    }
    restore(service, input, changes, plan.undo_of, skipped, notes).await
}

/// Eligible operations for this scope, or None with the explanatory note.
fn candidates(
    service: &WorkspaceCommands<'_>,
    input: &UndoInput,
    notes: &mut Vec<String>,
) -> Result<Option<Vec<JournalOp>>, WorkspaceError> {
    let wanted = input.ops.as_ref().map_or(input.steps, Vec::len);
    let query = UndoCandidateQuery {
        session_id: (input.scope == UndoScope::Session).then(|| service.session_id.clone()),
        limit: wanted,
        seqs: input.ops.clone(),
    };
    let candidates = service.journal.undo_candidates(&query).map_err(|error| {
        WorkspaceError::new(ErrorKind::EngineUnavailable, format!("journal: {error}"))
    })?;
    let scope = input.scope.as_str();
    if candidates.is_empty() {
        notes.push(match &input.ops {
            None => format!("undo: nothing to undo in {scope} scope"),
            Some(named) => format!(
                "undo: op(s) {} are not undoable in {scope} scope — already undone, an undo's own op, or another session's",
                join(named)
            ),
        });
        return Ok(None);
    }
    if candidates.len() < wanted {
        notes.push(match &input.ops {
            None => format!(
                "undo: only {} of the requested {} op(s) are undoable in {scope} scope",
                candidates.len(),
                input.steps
            ),
            Some(named) => {
                let missing: Vec<i64> = named
                    .iter()
                    .copied()
                    .filter(|seq| !candidates.iter().any(|op| op.seq == *seq))
                    .collect();
                format!(
                    "undo: op(s) {} are not undoable in {scope} scope and were skipped",
                    join(&missing)
                )
            }
        });
    }
    Ok(Some(candidates))
}

/// The restoration batch (or the bookkeeping op when no bytes differ) through
/// the pipeline. A refusal or failed application leaves the operations undoable.
async fn restore(
    service: &mut WorkspaceCommands<'_>,
    input: &UndoInput,
    changes: Vec<TransactionChange>,
    undo_of: Vec<i64>,
    skipped: Vec<UndoRow>,
    mut notes: Vec<String>,
) -> WorkspaceResponse<UndoRow> {
    let bookkeeping = changes.is_empty();
    let intent = match serde_json::to_vec(input) {
        Ok(intent) => intent,
        Err(error) => {
            return budget::failure(WorkspaceError::new(
                ErrorKind::PatternInvalid,
                error.to_string(),
            ));
        }
    };
    let request = TransactionRequest {
        operation: "undo".into(),
        serialized_request: intent,
        session_id: service.session_id.clone(),
        changes: changes.clone(),
        undo_of: undo_of.clone(),
        format: FormatMode::Disabled,
    };
    let reads = &service.reads;
    let mut transactions = match WorkspaceTransactions::new(
        service.policy,
        service.worker,
        service.journal,
        service.seal,
        service.options.clone(),
    ) {
        Ok(transactions) => transactions.with_formatter_tasks(service.formatter_tasks.clone()),
        Err(error) => {
            return budget::failure(super::transaction_errors::map(reads, error, &changes));
        }
    };
    let prepared = match transactions.prepare(request).await {
        Ok(prepared) => prepared,
        // A typed refusal (stale anchor race, parse rejection when the
        // restored bytes are broken, engine unavailable): nothing was applied
        // or journaled, and the operations remain undoable.
        Err(error) => {
            notes.push(format!(
                "undo: the restoration batch was refused; op(s) {} remain undoable",
                join(&undo_of)
            ));
            return WorkspaceResponse::Failure {
                version: EnvelopeVersion::LATEST,
                error: super::transaction_errors::map(reads, error, &changes),
                notes: Some(notes),
            };
        }
    };
    let token = prepared.confirmation_token().to_owned();
    let outcome = transactions.apply(prepared, &token).await;
    notes.extend(outcome.notes.iter().cloned());
    let rows: Vec<WriteReport> = outcome
        .files
        .iter()
        .map(|file| {
            WriteReport::from_preview(&file.preview, file.disposition, outcome.journal_intent_seq)
        })
        .collect();
    if outcome.status != TransactionStatus::Applied {
        for row in &rows {
            notes.push(format!("{}: {}", row.path, row.disposition));
        }
        notes.push(format!(
            "undo: the restoration did not apply; op(s) {} remain undoable",
            join(&undo_of)
        ));
        let error = outcome.error.map_or_else(
            || {
                WorkspaceError::new(
                    ErrorKind::EngineUnavailable,
                    "the restoration did not apply",
                )
            },
            |error| super::transaction_errors::map(reads, error, &changes),
        );
        return WorkspaceResponse::Failure {
            version: EnvelopeVersion::LATEST,
            error,
            notes: Some(notes),
        };
    }
    if bookkeeping {
        notes.push(format!(
            "undo: op(s) {} marked undone; no file content changed",
            join(&undo_of)
        ));
        return finish(input.budget, skipped, notes);
    }
    notes.push(format!(
        "undo: reverted op(s) {} in {} scope",
        join(&undo_of),
        input.scope.as_str()
    ));
    let mut rows: Vec<UndoRow> = rows.into_iter().map(UndoRow::Write).collect();
    rows.extend(skipped);
    finish(input.budget, rows, notes)
}

fn validate(input: &UndoInput) -> Result<(), WorkspaceError> {
    let invalid = |message: String| Err(WorkspaceError::new(ErrorKind::PatternInvalid, message));
    if input.steps < 1 || input.steps > STEPS_MAX {
        return invalid(format!(
            "undo: steps must be an integer in [1, {STEPS_MAX}]"
        ));
    }
    if let Some(ops) = &input.ops {
        if ops.is_empty() {
            return invalid("undo: ops must name at least one op".into());
        }
        if ops.len() > STEPS_MAX {
            return invalid(format!("undo: ops may name at most {STEPS_MAX} ops"));
        }
        if let Some(seq) = ops.iter().find(|seq| **seq < 1) {
            return invalid(format!(
                "undo: ops must be positive integer journal seqs, got {seq}"
            ));
        }
    }
    Ok(())
}

fn join(seqs: &[i64]) -> String {
    seqs.iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn finish(requested: u64, rows: Vec<UndoRow>, notes: Vec<String>) -> WorkspaceResponse<UndoRow> {
    budget::success(
        rows,
        requested,
        Freshness::Warm,
        notes,
        |row| match row {
            UndoRow::Write(report) => format!("path:{}", report.path),
            UndoRow::Conflict { path, .. }
            | UndoRow::Evicted { path, .. }
            | UndoRow::Kept { path, .. } => format!("path:{path}"),
        },
        vec!["raise budget".into()],
    )
}
