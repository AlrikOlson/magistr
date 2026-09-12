use super::{
    EditInput, EditOp, WorkspaceCommands, WriteReport,
    gate::{self, GateInput},
    resolution::Resolution,
    resolved::ResolvedWrites,
    source_reads::SourceReads,
};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, WorkspaceError, WorkspaceResponse},
    file_content::FileContent,
    query::{SymbolAddress, budget},
    transaction::{ExpectedState, FormatMode, TransactionChange},
    worker::WorkerSymbol,
};

pub(super) async fn run(
    service: &mut WorkspaceCommands<'_>,
    input: &EditInput,
) -> WorkspaceResponse<WriteReport> {
    if let Some(error) = schema_error(input) {
        return budget::failure(error);
    }
    let intent = match serde_json::to_vec(input) {
        Ok(intent) => intent,
        Err(error) => {
            return budget::failure(WorkspaceError::new(
                ErrorKind::PatternInvalid,
                error.to_string(),
            ));
        }
    };
    let resolution = match input.op {
        op if op.is_symbol_verb() => symbol_verb(service, input).await,
        EditOp::Rewrite => super::edit_rewrite::resolve(service, input).await,
        _ => replace_text(service, input),
    };
    let resolved = match resolution {
        Ok(Resolution::Writes(resolved)) => resolved,
        Ok(Resolution::NoOp(notes)) => return no_op(input.budget, notes),
        Ok(Resolution::Refused(error, notes)) => {
            return WorkspaceResponse::Failure {
                version: EnvelopeVersion::LATEST,
                error,
                notes: (!notes.is_empty()).then_some(notes),
            };
        }
        Err(error) => return budget::failure(error),
    };
    gate::run(
        service,
        GateInput {
            operation: "edit",
            intent,
            confirm: input.confirm.as_deref(),
            dry_run: input.dry_run,
            auto_apply: input.auto_apply,
            budget: input.budget,
            format: FormatMode::Enabled,
        },
        resolved,
    )
    .await
}

fn no_op(requested: u64, mut notes: Vec<String>) -> WorkspaceResponse<WriteReport> {
    notes.push("nothing to change: no file content differs; no write was attempted".into());
    budget::success(
        Vec::<WriteReport>::new(),
        requested,
        crate::contracts::Freshness::Warm,
        notes,
        |_| String::new(),
        Vec::new(),
    )
}

/// Schema-level misuse; the retained tool throws, the native verb refuses.
fn schema_error(input: &EditInput) -> Option<WorkspaceError> {
    let missing = |what: &str| {
        Some(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!("edit: {} requires {what}", input.op.as_str()),
        ))
    };
    let empty = |value: &Option<String>| value.as_deref().unwrap_or("").is_empty();
    if (input.op.is_symbol_verb() || input.op == EditOp::ReplaceText) && empty(&input.file) {
        return missing("file");
    }
    if input.op.is_symbol_verb() && empty(&input.symbol) {
        return missing("symbol");
    }
    if matches!(
        input.op,
        EditOp::ReplaceBody | EditOp::InsertAfter | EditOp::InsertBefore
    ) && input.content.is_none()
    {
        return missing("content");
    }
    if input.op == EditOp::Rewrite && (empty(&input.pattern) || input.rewrite_pattern.is_none()) {
        return missing("pattern and rewritePattern");
    }
    if input.op == EditOp::ReplaceText && (empty(&input.old) || input.new.is_none()) {
        return missing("old and new");
    }
    None
}

/// Read one target's text and check the caller's staleness anchor.
pub(super) fn read_anchored(
    reads: &SourceReads<'_>,
    file: &str,
    expected_hash: Option<&str>,
) -> Result<String, Resolution> {
    let content = reads
        .read(file)
        .map_err(Resolution::refuse)?
        .ok_or_else(|| {
            Resolution::refuse(WorkspaceError::new(
                ErrorKind::FileNotFound,
                format!(
                    "no such file in the workspace: {file:?}; check the path (search mode:files locates files by name)"
                ),
            ))
        })?;
    if let Some(expected) = expected_hash
        && content.content_hash() != expected
    {
        return Err(Resolution::refuse(WorkspaceError::new(
            ErrorKind::StaleAnchor {
                content_hash: content.content_hash(),
            },
            format!(
                "{file:?} changed since it was read (expectedHash mismatch); re-read the file and re-derive the edit against the returned contentHash"
            ),
        )));
    }
    match content {
        FileContent::Text(text) => Ok(text),
        FileContent::Binary(_) => Err(Resolution::refuse(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!(
                "{file:?} contains binary bytes or invalid UTF-8; text edits cannot preserve its bytes"
            ),
        ))),
    }
}

/// Split into lines, remembering whether the content ended with a newline.
pub(super) fn to_lines(content: &str) -> (Vec<&str>, bool) {
    let mut lines: Vec<_> = content.split('\n').collect();
    let trailing = lines.last() == Some(&"");
    if trailing {
        lines.pop();
    }
    (lines, trailing)
}

pub(super) fn from_lines(lines: &[&str], trailing: bool) -> String {
    let mut joined = lines.join("\n");
    if trailing {
        joined.push('\n');
    }
    joined
}

pub(super) fn text_change(path: &str, before: &str, after: String) -> TransactionChange {
    TransactionChange {
        path: path.into(),
        expected: ExpectedState::Hash(FileContent::Text(before.into()).content_hash()),
        content: Some(FileContent::Text(after)),
        baseline: None,
    }
}

async fn symbol_verb(
    service: &WorkspaceCommands<'_>,
    input: &EditInput,
) -> Result<Resolution, WorkspaceError> {
    let file = SourceReads::normalize(input.file.as_deref().unwrap_or(""));
    let before = match read_anchored(&service.reads, &file, input.expected_hash.as_deref()) {
        Ok(before) => before,
        Err(refusal) => return Ok(refusal),
    };
    let symbols = match outline(service, &file, &before) {
        Ok(symbols) => symbols,
        Err(refusal) => return Ok(refusal),
    };
    let address = input.symbol.as_deref().unwrap_or("");
    let resolved = match SymbolAddress::resolve(&symbols, address, &file) {
        Ok(resolved) => resolved,
        Err((error, notes)) => return Ok(Resolution::Refused(error, notes)),
    };
    let (lines, trailing) = to_lines(&before);
    let extent = SymbolAddress::extent(resolved, &symbols, lines.len());
    if input.op == EditOp::Rename {
        return super::edit_rename::resolve(service, input, &file, &resolved.name, &before).await;
    }
    let (body, _) = to_lines(input.content.as_deref().unwrap_or(""));
    let start = extent.start.max(1) - 1;
    let end = extent.end.min(lines.len());
    let mut next: Vec<&str> = Vec::with_capacity(lines.len() + body.len());
    match input.op {
        EditOp::ReplaceBody => {
            next.extend_from_slice(&lines[..start]);
            next.extend_from_slice(&body);
            next.extend_from_slice(&lines[end..]);
        }
        EditOp::InsertAfter => {
            next.extend_from_slice(&lines[..end]);
            next.extend_from_slice(&body);
            next.extend_from_slice(&lines[end..]);
        }
        _ => {
            next.extend_from_slice(&lines[..start]);
            next.extend_from_slice(&body);
            next.extend_from_slice(&lines[start..]);
        }
    }
    Ok(Resolution::Writes(ResolvedWrites {
        changes: vec![text_change(&file, &before, from_lines(&next, trailing))],
        notes: vec![format!(
            "{}: resolved {:?} ({}) to lines {}–{} of {file}",
            input.op.as_str(),
            resolved.name,
            resolved.kind,
            extent.start,
            extent.end
        )],
        always_gated: false,
    }))
}

/// Symbol addressing strictly requires the validated derived outline of the
/// exact bytes being edited; anything less answers INDEX_UNAVAILABLE.
fn outline(
    service: &WorkspaceCommands<'_>,
    file: &str,
    before: &str,
) -> Result<Vec<WorkerSymbol>, Resolution> {
    let unavailable = |progress: String| {
        Resolution::Refused(
            WorkspaceError::new(
                ErrorKind::IndexUnavailable,
                "edit with a symbol address requires the Tier-1 index and it is not up yet; retry once the build completes, or use replace_text",
            ),
            vec![progress],
        )
    };
    let Some(index) = service.index else {
        return Err(unavailable("no index exists yet for this workspace".into()));
    };
    let status = index.status();
    let progress = format!(
        "index progress: {}/{} parsed files, building={}",
        status.parsed, status.files, status.building
    );
    let mut query = index
        .file(file)
        .map_err(|error| unavailable(error.to_string()))?;
    if query.generation == 0 || query.truncated || query.rows.is_empty() {
        return Err(unavailable(progress));
    }
    let indexed = query.rows.remove(0);
    if indexed.content_hash != FileContent::Text(before.to_owned()).content_hash() {
        return Err(unavailable(format!(
            "{progress}; the outline and current file bytes differ, retry after refresh"
        )));
    }
    let mut symbols = indexed.symbols;
    symbols.sort_by_key(|row| (row.start_line, row.start_col));
    Ok(symbols)
}

fn replace_text(
    service: &WorkspaceCommands<'_>,
    input: &EditInput,
) -> Result<Resolution, WorkspaceError> {
    let file = SourceReads::normalize(input.file.as_deref().unwrap_or(""));
    let before = match read_anchored(&service.reads, &file, input.expected_hash.as_deref()) {
        Ok(before) => before,
        Err(refusal) => return Ok(refusal),
    };
    let old = input.old.as_deref().unwrap_or("");
    let replacement = input.new.as_deref().unwrap_or("");
    let indices: Vec<usize> = before.match_indices(old).map(|(at, _)| at).collect();
    let stale = |message: String| {
        Ok(Resolution::refuse(WorkspaceError::new(
            ErrorKind::StaleAnchor {
                content_hash: FileContent::Text(before.clone()).content_hash(),
            },
            message,
        )))
    };
    if indices.is_empty() {
        return stale(format!(
            "replace_text: the anchor text was not found in {file:?}; the file may have changed since it was read — re-read it and re-derive the edit against the returned contentHash"
        ));
    }
    if let Some(occurrence) = input.occurrence
        && (occurrence == 0 || occurrence > indices.len())
    {
        return stale(format!(
            "replace_text: occurrence {occurrence} was requested but the anchor text appears {} time(s) in {file:?}; re-read the file and re-derive the edit against the returned contentHash",
            indices.len()
        ));
    }
    let (after, note) = match input.occurrence {
        None => (
            before.replace(old, replacement),
            format!(
                "replace_text: replaced all {} occurrence(s) in {file}",
                indices.len()
            ),
        ),
        Some(occurrence) => {
            let at = indices[occurrence - 1];
            (
                format!(
                    "{}{replacement}{}",
                    &before[..at],
                    &before[at + old.len()..]
                ),
                format!(
                    "replace_text: replaced occurrence {occurrence} of {} in {file}",
                    indices.len()
                ),
            )
        }
    };
    Ok(Resolution::Writes(ResolvedWrites {
        changes: vec![text_change(&file, &before, after)],
        notes: vec![note],
        always_gated: false,
    }))
}
