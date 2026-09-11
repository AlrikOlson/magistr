use super::{
    CONFIRM_DIFF_CHARS, CONFIRM_LINE_LIMIT, WorkspaceCommands, WriteReport, diff,
    resolved::ResolvedWrites, source_reads::SourceReads, transaction_errors,
};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    query::budget,
    transaction::{
        FileDisposition, FilePreview, FormatMode, TransactionChange, TransactionOutcome,
        TransactionRequest, TransactionStatus, WorkspaceTransactions,
    },
};

/// How one resolved edit or files call is admitted through the pipeline.
pub(super) struct GateInput<'i> {
    pub operation: &'static str,
    pub intent: Vec<u8>,
    pub confirm: Option<&'i str>,
    pub dry_run: bool,
    pub auto_apply: bool,
    pub budget: u64,
}

/// Prepare the exact plan, apply the retained confirm gate over its token,
/// then preview or apply. Refusals carry the taxonomy payloads; a failed
/// application reports every observed file effect instead of a bare error.
pub(super) async fn run(
    service: &mut WorkspaceCommands<'_>,
    gate: GateInput<'_>,
    resolved: ResolvedWrites,
) -> WorkspaceResponse<WriteReport> {
    let ResolvedWrites {
        changes,
        mut notes,
        always_gated,
    } = resolved;
    // A verb that resolved to no writes has nothing to prepare or seal: the
    // answer is the nothing-to-change envelope, unless a confirm was presented
    // for a plan that has since evaporated (a stale token fails closed).
    if changes.is_empty() {
        if gate.confirm.is_some() {
            return budget::failure(WorkspaceError::new(
                ErrorKind::ConfirmMismatch,
                "confirm: the token was minted for a plan that no longer produces any change; re-run the call without confirm",
            ));
        }
        return nothing_to_change(gate.budget, notes);
    }
    let request = TransactionRequest {
        operation: gate.operation.into(),
        serialized_request: gate.intent,
        session_id: service.session_id.clone(),
        changes: changes.clone(),
        undo_of: Vec::new(),
        format: FormatMode::Enabled,
    };
    let reads = &service.reads;
    let mut transactions = match WorkspaceTransactions::new(
        service.policy,
        service.worker,
        service.journal,
        service.seal,
        service.options.clone(),
    ) {
        Ok(transactions) => transactions,
        Err(error) => return budget::failure(transaction_errors::map(reads, error, &changes)),
    };
    let prepared = match transactions.prepare(request).await {
        Ok(prepared) => prepared,
        Err(error) => return budget::failure(transaction_errors::map(reads, error, &changes)),
    };
    let token = prepared.confirmation_token().to_owned();
    let confirmed = match verify_confirm(gate.confirm, &token) {
        Ok(confirmed) => confirmed,
        Err(error) => return budget::failure(error),
    };
    let previews = prepared.preview();
    let changed: Vec<&FilePreview> = previews
        .iter()
        .filter(|preview| preview.before != preview.after)
        .collect();
    if changed.is_empty() {
        return nothing_to_change(gate.budget, notes);
    }
    let changed_lines: usize = changed
        .iter()
        .map(|preview| diff::changed_lines(preview.before.as_ref(), preview.after.as_ref()))
        .sum();
    let gated = always_gated || changed.len() > 1 || changed_lines > CONFIRM_LINE_LIMIT;
    if gated && !gate.dry_run && !confirmed && !gate.auto_apply {
        return confirm_required(gate.operation, token, &changed, changed_lines, notes);
    }
    if gate.dry_run {
        let rows = previews
            .iter()
            .map(|preview| WriteReport::from_preview(preview, FileDisposition::NotApplied, None))
            .collect();
        notes.push("dry run: the full pipeline ran through verification and formatting; nothing was written or journaled".into());
        return success(rows, gate.budget, notes);
    }
    let outcome = transactions.apply(prepared, &token).await;
    report(reads, outcome, &changes, gate.budget, notes)
}

/// A presented confirm is verified before anything else, even before the
/// nothing-to-change answer: a rewrite whose matches evaporated is exactly
/// what a stale confirm looks like. The native seal binds the session, the
/// exact intent bytes and the finalized plan together, so any difference
/// (files moved, arguments changed, foreign key) is a mismatch.
fn verify_confirm(presented: Option<&str>, token: &str) -> Result<bool, WorkspaceError> {
    match presented {
        None => Ok(false),
        Some(presented) if presented == token => Ok(true),
        Some(presented) => {
            let message = if presented.starts_with("mgw2.") {
                "confirm: the token does not authenticate this owner generation, session, intent and finalized plan — the confirmed files changed after it was minted, the call's arguments differ, or it was minted under another key; re-run the call without confirm to review the fresh diff and get a new token"
            } else {
                "confirm: the token is not a native mgw2 confirmation (earlier wb1 and mgw1 tokens are invalidated); re-run the call without confirm for a fresh diff and token"
            };
            Err(WorkspaceError::new(ErrorKind::ConfirmMismatch, message))
        }
    }
}

fn confirm_required(
    operation: &str,
    token: String,
    changed: &[&FilePreview],
    changed_lines: usize,
    mut notes: Vec<String>,
) -> WorkspaceResponse<WriteReport> {
    let mut spent = 0;
    for preview in changed {
        let rendered = diff::unified_diff(
            &preview.path,
            preview.before.as_ref(),
            preview.after.as_ref(),
        );
        if spent + rendered.len() > CONFIRM_DIFF_CHARS {
            notes.push(format!(
                "…aggregated diff truncated at {CONFIRM_DIFF_CHARS} chars ({} file(s) total); dryRun:true returns every diff in full",
                changed.len()
            ));
            break;
        }
        spent += rendered.len();
        notes.push(rendered);
    }
    WorkspaceResponse::Failure {
        version: EnvelopeVersion::LATEST,
        error: WorkspaceError::new(
            ErrorKind::ConfirmRequired {
                confirm_token: token,
            },
            format!(
                "this {operation} touches {} file(s) / {changed_lines} changed line(s) and is confirm-gated; review the aggregated diff in notes, then re-invoke the identical call with confirm set to the confirmToken to apply (dryRun:true previews in full; autoApply:true is the trusted-harness bypass)",
                changed.len()
            ),
        ),
        notes: Some(notes),
    }
}

/// Rows in input order from the observed outcome; a failed application keeps
/// every per-file effect and the resolved intent state visible in the notes.
fn report(
    reads: &SourceReads<'_>,
    outcome: TransactionOutcome,
    changes: &[TransactionChange],
    requested: u64,
    mut notes: Vec<String>,
) -> WorkspaceResponse<WriteReport> {
    let rows: Vec<_> = outcome
        .files
        .iter()
        .map(|file| {
            WriteReport::from_preview(&file.preview, file.disposition, outcome.journal_intent_seq)
        })
        .collect();
    notes.extend(outcome.notes.iter().cloned());
    if outcome.status == TransactionStatus::Applied {
        return success(rows, requested, notes);
    }
    for row in &rows {
        notes.push(format!("{}: {}", row.path, row.disposition));
    }
    if let Some(seq) = outcome.journal_intent_seq {
        notes.push(format!(
            "journal intent {seq} was resolved as {}; it is not an applied revision",
            match outcome.status {
                TransactionStatus::Failed => "rolled back",
                _ => "indeterminate",
            }
        ));
    }
    let error = outcome.error.map_or_else(
        || {
            WorkspaceError::new(
                ErrorKind::EngineUnavailable,
                "the transaction did not apply; see the per-file effects in notes",
            )
        },
        |error| transaction_errors::map(reads, error, changes),
    );
    WorkspaceResponse::Failure {
        version: EnvelopeVersion::LATEST,
        error,
        notes: Some(notes),
    }
}

fn nothing_to_change(requested: u64, mut notes: Vec<String>) -> WorkspaceResponse<WriteReport> {
    notes.push("nothing to change: no file content differs; no write was attempted".into());
    success(Vec::new(), requested, notes)
}

fn success(
    rows: Vec<WriteReport>,
    requested: u64,
    notes: Vec<String>,
) -> WorkspaceResponse<WriteReport> {
    budget::success(
        rows,
        requested,
        Freshness::Warm,
        notes,
        |row| format!("path:{}", row.path),
        vec!["raise budget".into()],
    )
}
