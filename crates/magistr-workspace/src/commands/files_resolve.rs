use super::{
    FilesInput, FilesOp, WorkspaceCommands, WriteReport,
    gate::{self, GateInput},
    move_imports,
    resolution::Resolution,
    resolved::ResolvedWrites,
    rust_move,
    source_reads::SourceReads,
};
use crate::{
    contracts::{EnvelopeVersion, ErrorKind, Freshness, WorkspaceError, WorkspaceResponse},
    file_content::FileContent,
    query::budget,
    transaction::{BaselineSource, ExpectedState, TransactionChange},
};

pub(super) async fn run(
    service: &mut WorkspaceCommands<'_>,
    input: &FilesInput,
) -> WorkspaceResponse<WriteReport> {
    let from = SourceReads::normalize(input.from.as_deref().unwrap_or(""));
    let to = SourceReads::normalize(input.to.as_deref().unwrap_or(""));
    let needs_from = matches!(input.op, FilesOp::Move | FilesOp::Copy | FilesOp::Delete);
    if needs_from && from.is_empty() || input.op != FilesOp::Delete && to.is_empty() {
        return budget::failure(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            format!(
                "files: {} requires {}",
                input.op.as_str(),
                if needs_from && from.is_empty() {
                    "from"
                } else {
                    "to"
                }
            ),
        ));
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
        FilesOp::Move => resolve_move(service, input, &from, &to),
        FilesOp::Copy => resolve_copy(service, input, &from, &to),
        FilesOp::Create => resolve_create(service, &to),
        FilesOp::Delete => resolve_delete(service, input, &from),
        FilesOp::Mkdir => resolve_mkdir(service, &to),
    };
    let resolved = match resolution {
        Ok(Resolution::Writes(resolved)) => resolved,
        Ok(Resolution::NoOp(mut notes)) => {
            notes.push("nothing to change: no write was attempted".into());
            return budget::success(
                Vec::<WriteReport>::new(),
                input.budget,
                Freshness::Warm,
                notes,
                |_| String::new(),
                Vec::new(),
            );
        }
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
            operation: "files",
            intent,
            confirm: input.confirm.as_deref(),
            dry_run: input.dry_run,
            auto_apply: input.auto_apply,
            budget: input.budget,
        },
        resolved,
    )
    .await
}

/// Read `from` and check the caller's staleness anchor.
fn read_source(
    service: &WorkspaceCommands<'_>,
    op: &str,
    from: &str,
    expected_hash: Option<&str>,
) -> Result<FileContent, Resolution> {
    let content = service
        .reads
        .read(from)
        .map_err(Resolution::refuse)?
        .ok_or_else(|| {
            Resolution::refuse(WorkspaceError::new(
                ErrorKind::FileNotFound,
                format!(
                    "files.{op}: no such file in the workspace: {from:?}; check the path (search mode:files locates files by name) — directories are not movable/copyable in v1, only files"
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
                "{from:?} changed since it was read (expectedHash mismatch); re-read the file and re-derive the operation against the returned contentHash"
            ),
        )));
    }
    Ok(content)
}

fn expected(content: Option<&FileContent>) -> ExpectedState {
    content.map_or(ExpectedState::Missing, |content| {
        ExpectedState::Hash(content.content_hash())
    })
}

fn same_path(op: &str, from: &str) -> Resolution {
    Resolution::refuse(WorkspaceError::new(
        ErrorKind::PatternInvalid,
        format!("files.{op}: from and to name the same path {from:?}"),
    ))
}

fn resolve_move(
    service: &WorkspaceCommands<'_>,
    input: &FilesInput,
    from: &str,
    to: &str,
) -> Result<Resolution, WorkspaceError> {
    if from == to {
        return Ok(same_path("move", from));
    }
    let source = match read_source(service, "move", from, input.expected_hash.as_deref()) {
        Ok(source) => source,
        Err(refusal) => return Ok(refusal),
    };
    let target = service.reads.read(to)?;
    // Opaque files have no source imports: skip text rewriting and the
    // workspace-wide importer scan, but keep the same gated write batch.
    let listing = match &source {
        FileContent::Text(_) => service.queries()?.files()?,
        FileContent::Binary(_) => Vec::new(),
    };
    let mut notes = Vec::new();
    let plan = move_imports::plan(&service.reads, &listing, from, to, &source);
    let rust = match &source {
        FileContent::Text(text) if from.ends_with(".rs") => {
            let read = |path: &str| match service.reads.read(path) {
                Ok(Some(FileContent::Text(text))) => Some(text),
                _ => None,
            };
            Some(rust_move::plan_rust_move(from, to, text, &listing, &read))
        }
        _ => None,
    };
    let moved = match (&rust, &source) {
        (Some(rust), _) => FileContent::Text(rust.moved_content.clone()),
        (None, FileContent::Text(_)) => FileContent::Text(plan.moved_content.clone()),
        (None, binary) => binary.clone(),
    };
    let mut changes = vec![
        TransactionChange {
            path: to.into(),
            expected: expected(target.as_ref()),
            content: Some(moved),
            // Relocating an already-broken file reports its diagnostics
            // instead of rejecting as a new error: the source is the baseline.
            baseline: Some(BaselineSource {
                path: from.into(),
                expected_hash: source.content_hash(),
            }),
        },
        TransactionChange {
            path: from.into(),
            expected: ExpectedState::Hash(source.content_hash()),
            content: None,
            baseline: None,
        },
    ];
    if target.is_some() {
        notes.push(format!(
            "move: {to:?} already exists and will be overwritten"
        ));
    }
    if let Some(rust) = rust {
        for write in rust.writes {
            if write.path == from || write.path == to {
                continue;
            }
            changes.push(TransactionChange {
                path: write.path,
                expected: ExpectedState::Hash(FileContent::Text(write.before).content_hash()),
                content: Some(FileContent::Text(write.content)),
                baseline: None,
            });
        }
        let mut leading = vec![format!("move {from:?} → {to:?}")];
        leading.extend(rust.notes);
        leading.append(&mut notes);
        return Ok(Resolution::Writes(ResolvedWrites {
            changes,
            notes: leading,
            always_gated: true,
        }));
    }
    for importer in plan.importers {
        changes.push(TransactionChange {
            path: importer.path,
            expected: ExpectedState::Hash(FileContent::Text(importer.before).content_hash()),
            content: Some(FileContent::Text(importer.content)),
            baseline: None,
        });
    }
    notes.insert(
        0,
        format!(
            "move {from:?} → {to:?}: rewrote {} import specifier(s) — {} in the moved file, the rest across {} importer file(s); specifier-based and name-resolution-free (live scan; go imports name packages, not files, and are not rewritten)",
            plan.specifiers_rewritten, plan.moved_count, plan.importers_rewritten
        ),
    );
    Ok(Resolution::Writes(ResolvedWrites {
        changes,
        notes,
        always_gated: true,
    }))
}

fn resolve_copy(
    service: &WorkspaceCommands<'_>,
    input: &FilesInput,
    from: &str,
    to: &str,
) -> Result<Resolution, WorkspaceError> {
    if from == to {
        return Ok(same_path("copy", from));
    }
    let source = match read_source(service, "copy", from, input.expected_hash.as_deref()) {
        Ok(source) => source,
        Err(refusal) => return Ok(refusal),
    };
    let target = service.reads.read(to)?;
    if target.as_ref() == Some(&source) {
        return Ok(Resolution::NoOp(vec![format!(
            "copy: {to:?} already matches the source"
        )]));
    }
    let overwriting = target.is_some();
    Ok(Resolution::Writes(ResolvedWrites {
        changes: vec![TransactionChange {
            path: to.into(),
            expected: expected(target.as_ref()),
            baseline: Some(BaselineSource {
                path: from.into(),
                expected_hash: source.content_hash(),
            }),
            content: Some(source),
        }],
        notes: vec![format!(
            "copy {from:?} → {to:?}{}",
            if overwriting {
                " (overwrites the existing target)"
            } else {
                ""
            }
        )],
        always_gated: overwriting,
    }))
}

fn resolve_create(service: &WorkspaceCommands<'_>, to: &str) -> Result<Resolution, WorkspaceError> {
    if service.reads.read(to)?.is_some() {
        return Ok(Resolution::NoOp(vec![format!(
            "create: {to:?} already exists and was left untouched (touch semantics; use edit to change its content)"
        )]));
    }
    Ok(Resolution::Writes(ResolvedWrites {
        changes: vec![TransactionChange {
            path: to.into(),
            expected: ExpectedState::Missing,
            content: Some(FileContent::Text(String::new())),
            baseline: None,
        }],
        notes: vec![format!("create: new empty file {to:?}")],
        always_gated: false,
    }))
}

fn resolve_delete(
    service: &WorkspaceCommands<'_>,
    input: &FilesInput,
    from: &str,
) -> Result<Resolution, WorkspaceError> {
    let source = match read_source(service, "delete", from, input.expected_hash.as_deref()) {
        Ok(source) => source,
        Err(refusal) => return Ok(refusal),
    };
    Ok(Resolution::Writes(ResolvedWrites {
        changes: vec![TransactionChange {
            path: from.into(),
            expected: ExpectedState::Hash(source.content_hash()),
            content: None,
            baseline: None,
        }],
        notes: vec![format!(
            "delete {from:?}: journal-backed move to .workbench/trash/ — recoverable with undo"
        )],
        always_gated: false,
    }))
}

fn resolve_mkdir(service: &WorkspaceCommands<'_>, to: &str) -> Result<Resolution, WorkspaceError> {
    let keep = format!("{to}/.gitkeep");
    if service.reads.read(&keep)?.is_some() {
        return Ok(Resolution::NoOp(vec![format!(
            "mkdir: {to:?} already exists"
        )]));
    }
    Ok(Resolution::Writes(ResolvedWrites {
        changes: vec![TransactionChange {
            path: keep.clone(),
            expected: ExpectedState::Missing,
            content: Some(FileContent::Text(String::new())),
            baseline: None,
        }],
        notes: vec![format!(
            "mkdir: materialized {to:?} as a journaled {keep:?} — the journal's file model has no directory entity, so the marker file is what makes the operation journaled and undoable"
        )],
        always_gated: false,
    }))
}
