use super::{
    ExpectedState, FilePreview, FormatMode, PreparedTransaction, TransactionError,
    TransactionRequest, VerificationStatus, WorkspaceTransactions, binding,
    verification::Verification,
};
use crate::file_content::FileContent;
use std::collections::BTreeSet;

pub(super) async fn prepare(
    service: &mut WorkspaceTransactions<'_>,
    mut request: TransactionRequest,
) -> Result<PreparedTransaction, TransactionError> {
    validate(&request)?;
    let mut prepared = PreparedTransaction {
        request: request.clone(),
        preview: Vec::new(),
        token: String::new(),
        paths: Vec::new(),
        snapshots: Vec::new(),
        baselines: Vec::new(),
    };
    let mut seen = BTreeSet::new();
    let mut total = request.serialized_request.len();
    for change in &mut request.changes {
        let path = service.files.resolve(service.policy, &change.path)?;
        if !seen.insert(path.clone()) {
            return Err(TransactionError::InvalidInput(
                "duplicate resolved target".into(),
            ));
        }
        let before = service
            .files
            .snapshot(service.policy, &change.path, &path)?;
        anchor(&change.path, &change.expected, &before.content)?;
        if before.content.is_none() && change.content.is_none() {
            return Err(TransactionError::InvalidInput(format!(
                "{}: cannot delete missing file",
                change.path
            )));
        }
        // Classification follows bytes, never the caller's chosen enum variant.
        let after = change
            .content
            .take()
            .map(|content| FileContent::decode(content.as_bytes().to_vec()));
        total = total
            .saturating_add(size(&before.content))
            .saturating_add(size(&after));
        if total > 16 * 1024 * 1024 {
            return Err(TransactionError::InvalidInput(
                "batch exceeds 16 MiB hydrated budget".into(),
            ));
        }
        let baseline = if let Some(source) = &change.baseline {
            let admitted = service.policy.check_read(&source.path)?;
            let snapshot = service
                .files
                .baseline(service.policy, &source.path, &admitted)?;
            anchor(
                &source.path,
                &ExpectedState::Hash(source.expected_hash.clone()),
                &snapshot.content,
            )?;
            total = total.saturating_add(size(&snapshot.content));
            if total > 16 * 1024 * 1024 {
                return Err(TransactionError::InvalidInput(
                    "batch exceeds 16 MiB hydrated budget".into(),
                ));
            }
            prepared
                .baselines
                .push((source.path.clone(), admitted, snapshot));
            &prepared
                .baselines
                .last()
                .expect("baseline just appended")
                .2
                .content
        } else {
            &before.content
        };
        let relative = path
            .strip_prefix(&service.files.root)
            .map_err(|_| {
                TransactionError::InvalidInput("resolved target outside workspace".into())
            })?
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| TransactionError::InvalidInput("target is not UTF-8".into()))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        let mut preview = FilePreview {
            path: relative,
            before: before.content.clone(),
            after,
            diagnostics: vec![],
            verification: VerificationStatus::Unchanged,
            formatting: "unchanged".into(),
        };
        if preview.before != preview.after {
            verify_candidate(service, &mut preview, baseline).await?;
            let unformatted_size = size(&preview.after);
            format(service, request.format, &mut preview).await?;
            total = total
                .saturating_sub(unformatted_size)
                .saturating_add(size(&preview.after));
            if total > 16 * 1024 * 1024 {
                return Err(TransactionError::InvalidInput(
                    "formatted batch exceeds 16 MiB hydrated budget".into(),
                ));
            }
        }
        prepared.paths.push(path);
        prepared.snapshots.push(before);
        prepared.preview.push(preview);
    }
    let final_bytes = prepared
        .preview
        .iter()
        .map(|file| size(&file.before).saturating_add(size(&file.after)))
        .chain(
            prepared
                .baselines
                .iter()
                .map(|(_, _, snapshot)| size(&snapshot.content)),
        )
        .fold(
            prepared.request.serialized_request.len(),
            usize::saturating_add,
        );
    if final_bytes > 16 * 1024 * 1024 {
        return Err(TransactionError::InvalidInput(
            "formatted batch exceeds 16 MiB hydrated budget".into(),
        ));
    }
    let bytes = binding::bytes(&prepared);
    prepared.token = service.seal.seal(
        &prepared.request.session_id,
        &prepared.request.operation,
        &prepared.request.serialized_request,
        &bytes,
    );
    Ok(prepared)
}
fn validate(request: &TransactionRequest) -> Result<(), TransactionError> {
    if (request.changes.is_empty() && request.undo_of.is_empty())
        || request.changes.len() > 256
        || request.undo_of.len() > 1000
        || request.undo_of.iter().any(|id| *id <= 0)
        || (!request.undo_of.is_empty() && request.format != FormatMode::Disabled)
    {
        return Err(TransactionError::InvalidInput("requires at most 256 changes (empty only for undo bookkeeping), positive bounded undo links and formatting disabled for undo".into()));
    }
    if request.operation.is_empty()
        || request.session_id.is_empty()
        || request.operation.len() > 4096
        || request.session_id.len() > 4096
        || request.serialized_request.len() > 1024 * 1024
    {
        return Err(TransactionError::InvalidInput(
            "operation/session or serialized request exceeds limits".into(),
        ));
    }
    let mut bytes = request.serialized_request.len();
    for change in &request.changes {
        if change.path.is_empty()
            || change.path.len() > 4096
            || change
                .content
                .as_ref()
                .is_some_and(|c| c.as_bytes().len() > 4 * 1024 * 1024)
            || change
                .baseline
                .as_ref()
                .is_some_and(|b| b.path.len() > 4096 || b.expected_hash.len() != 64)
            || matches!(&change.expected,ExpectedState::Hash(hash) if hash.len()!=64 || !hash.bytes().all(|b|b.is_ascii_hexdigit()))
        {
            return Err(TransactionError::InvalidInput(
                "invalid path, anchor, baseline, or 4 MiB file bound".into(),
            ));
        }
        bytes = bytes
            .saturating_add(change.path.len())
            .saturating_add(size(&change.content));
    }
    if bytes > 16 * 1024 * 1024 {
        return Err(TransactionError::InvalidInput(
            "batch exceeds 16 MiB input budget".into(),
        ));
    }
    Ok(())
}
fn size(content: &Option<FileContent>) -> usize {
    content
        .as_ref()
        .map_or(0, |content| content.as_bytes().len())
}
fn anchor(
    path: &str,
    expected: &ExpectedState,
    content: &Option<FileContent>,
) -> Result<(), TransactionError> {
    let matches = match (expected, content) {
        (ExpectedState::Missing, None) => true,
        (ExpectedState::Hash(hash), Some(content)) => *hash == content.content_hash(),
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(TransactionError::StaleAnchor(path.into()))
    }
}
async fn verify_candidate(
    service: &WorkspaceTransactions<'_>,
    preview: &mut FilePreview,
    baseline: &Option<FileContent>,
) -> Result<(), TransactionError> {
    let Some(after) = &preview.after else {
        preview.verification = VerificationStatus::Deletion;
        return Ok(());
    };
    if matches!(after, FileContent::Binary(_)) {
        preview.verification = VerificationStatus::Binary;
        return Ok(());
    }
    if !Verification::supported(&preview.path) {
        preview.verification = VerificationStatus::Unsupported;
        preview
            .diagnostics
            .push("unsupported text format; no parser verification was available".into());
        return Ok(());
    }
    preview.verification = VerificationStatus::Verified;
    let baseline = if let Some(baseline) = baseline {
        Verification::verify(service.worker, &preview.path, baseline).await?
    } else {
        None
    };
    if let Some(message) = Verification::verify(service.worker, &preview.path, after).await? {
        if baseline.is_none() {
            return Err(TransactionError::ParseRejected {
                path: preview.path.clone(),
                message,
            });
        }
        preview.verification = VerificationStatus::ExistingDiagnostics;
        preview.diagnostics.push(format!(
            "pre-existing broken baseline; remaining diagnostics: {message}"
        ));
    }
    Ok(())
}
async fn format(
    service: &WorkspaceTransactions<'_>,
    mode: FormatMode,
    preview: &mut FilePreview,
) -> Result<(), TransactionError> {
    let Some(FileContent::Text(source)) = &preview.after else {
        preview.formatting = "skipped: binary or deletion".into();
        return Ok(());
    };
    if mode == FormatMode::Disabled || service.policy.config().effective["format.enabled"] == false
    {
        preview.formatting = "disabled: exact bytes".into();
        return Ok(());
    }
    if preview.verification == VerificationStatus::Unsupported {
        preview.formatting = "skipped: unsupported parse verification; exact bytes".into();
        return Ok(());
    }
    if !preview.diagnostics.is_empty() {
        preview.formatting = "skipped: pre-existing syntax diagnostics".into();
        return Ok(());
    }
    let formatted = match service.formatter.format(&preview.path, source).await {
        Ok(formatted) => formatted,
        Err(error) => {
            let error: String = error.chars().take(2048).collect();
            preview.formatting = format!("degraded: verified unformatted content ({error})");
            return Ok(());
        }
    };
    if formatted == *source {
        preview.formatting = "formatter returned unchanged verified content".into();
        return Ok(());
    }
    if formatted.len() > 4 * 1024 * 1024 {
        preview.formatting =
            "degraded: verified unformatted content (formatter exceeded file bound)".into();
        return Ok(());
    }
    let content = FileContent::decode(formatted.into_bytes());
    if matches!(&content, FileContent::Binary(_)) {
        preview.formatting =
            "degraded: verified unformatted content (formatter returned binary)".into();
        return Ok(());
    }
    match Verification::verify(service.worker, &preview.path, &content).await? {
        Some(message) => {
            preview.formatting = format!(
                "degraded: verified unformatted content (formatter broke syntax: {message})"
            )
        }
        None => {
            preview.after = Some(content);
            preview.formatting = "formatted and reverified".into();
        }
    }
    Ok(())
}
