use super::{
    FileDisposition, FileResult, PreparedTransaction, TransactionError, TransactionOutcome,
    TransactionStatus, WorkspaceTransactions, binding, compensate,
};
use crate::{
    file_content::FileContent,
    journal::{JournalFileChange, JournalFileKind, JournalOpInit, JournalWriteOutcome},
};

pub(super) fn apply(
    service: &mut WorkspaceTransactions<'_>,
    prepared: PreparedTransaction,
    token: &str,
) -> TransactionOutcome {
    let mut result = TransactionOutcome {
        status: TransactionStatus::Failed,
        files: prepared
            .preview
            .iter()
            .map(|preview| FileResult {
                preview: preview.clone(),
                disposition: if preview.before == preview.after {
                    FileDisposition::Unchanged
                } else {
                    FileDisposition::NotApplied
                },
            })
            .collect(),
        journal_intent_seq: None,
        error: None,
        notes: vec![],
    };
    if let Err(error) = admit(service, &prepared, token) {
        result.error = Some(error);
        return result;
    }
    let files: Vec<_> = prepared
        .preview
        .iter()
        .filter(|p| p.before != p.after)
        .map(|preview| JournalFileChange {
            kind: match (&preview.before, &preview.after) {
                (None, _) => JournalFileKind::Create,
                (_, None) => JournalFileKind::Delete,
                _ => JournalFileKind::Write,
            },
            path: preview.path.clone(),
            before: preview
                .before
                .clone()
                .unwrap_or_else(|| FileContent::Binary(vec![])),
            after: preview
                .after
                .clone()
                .unwrap_or_else(|| FileContent::Binary(vec![])),
        })
        .collect();
    if files.is_empty() && prepared.request.undo_of.is_empty() {
        result.status = TransactionStatus::Applied;
        return result;
    }
    let intent = JournalOpInit {
        tool: prepared.request.operation.clone(),
        args_hash: FileContent::Binary(prepared.request.serialized_request.clone()).content_hash(),
        session_id: prepared.request.session_id.clone(),
        files,
        undo_of: (!prepared.request.undo_of.is_empty()).then(|| prepared.request.undo_of.clone()),
    };
    let appended = match service.journal.append_intent(&intent) {
        Ok(appended) => appended,
        Err(error) => {
            result.error = Some(TransactionError::Journal(error.to_string()));
            return result;
        }
    };
    let seq = appended.op.seq;
    result.journal_intent_seq = Some(seq);
    result.notes.extend(appended.notes);
    result.notes.push("journal sequence records durable intent; the receipt and resolved outcome determine application".into());
    let mut attempted = vec![false; prepared.preview.len()];
    let mut created = vec![Vec::new(); prepared.preview.len()];
    for (index, preview) in prepared.preview.iter().enumerate() {
        if preview.before == preview.after {
            continue;
        }
        if let Err(error) = swap(
            service,
            &prepared,
            index,
            &mut attempted[index],
            &mut created[index],
        ) {
            result.error = Some(error);
            compensate::compensate(service, &prepared, &attempted, &created, &mut result);
            let outcome = if result.status == TransactionStatus::Failed {
                JournalWriteOutcome::RolledBack
            } else {
                JournalWriteOutcome::Indeterminate
            };
            resolve(service, seq, outcome, &mut result);
            return result;
        }
        result.files[index].disposition = FileDisposition::Applied;
    }
    result.status = TransactionStatus::Applied;
    resolve(service, seq, JournalWriteOutcome::Applied, &mut result);
    result
}
fn admit(
    service: &WorkspaceTransactions<'_>,
    prepared: &PreparedTransaction,
    token: &str,
) -> Result<(), TransactionError> {
    let bytes = binding::bytes(prepared);
    service
        .seal
        .verify(
            token,
            &prepared.request.session_id,
            &prepared.request.operation,
            &prepared.request.serialized_request,
            &bytes,
        )
        .map_err(|error| TransactionError::Confirmation(error.to_string()))?;
    for (index, change) in prepared.request.changes.iter().enumerate() {
        let current =
            service
                .files
                .snapshot(service.policy, &change.path, &prepared.paths[index])?;
        if !current.same(&prepared.snapshots[index]) {
            return Err(TransactionError::StaleAnchor(change.path.clone()));
        }
    }
    for (path, admitted, snapshot) in &prepared.baselines {
        if !service
            .files
            .baseline(service.policy, path, admitted)?
            .same(snapshot)
        {
            return Err(TransactionError::StaleAnchor(path.clone()));
        }
    }
    Ok(())
}
fn swap(
    service: &WorkspaceTransactions<'_>,
    prepared: &PreparedTransaction,
    index: usize,
    attempted: &mut bool,
    created: &mut Vec<std::path::PathBuf>,
) -> Result<(), TransactionError> {
    let change = &prepared.request.changes[index];
    let snapshot = &prepared.snapshots[index];
    let path = &prepared.paths[index];
    let current = service.files.snapshot(service.policy, &change.path, path)?;
    if !current.same(snapshot) {
        return Err(TransactionError::StaleAnchor(change.path.clone()));
    }
    #[cfg(test)]
    if service.faults.swap_at == Some(index) {
        return Err(TransactionError::Io("injected atomic swap failure".into()));
    }
    *attempted = true;
    service.files.replace(
        service.policy,
        &change.path,
        path,
        prepared.preview[index].after.as_ref(),
        snapshot.permissions.as_ref(),
        created,
    )?;
    #[cfg(test)]
    if service.faults.after_swap_at == Some(index) {
        return Err(TransactionError::Io(
            "injected post-rename sync failure".into(),
        ));
    }
    let observed = service.files.snapshot(service.policy, &change.path, path)?;
    if observed.content != prepared.preview[index].after
        || (observed.content.is_some()
            && snapshot.permissions.is_some()
            && observed.permissions != snapshot.permissions)
    {
        return Err(TransactionError::Io(format!(
            "{}: swapped bytes or permissions failed read-back",
            change.path
        )));
    }
    Ok(())
}
fn resolve(
    service: &mut WorkspaceTransactions<'_>,
    seq: i64,
    outcome: JournalWriteOutcome,
    result: &mut TransactionOutcome,
) {
    #[cfg(test)]
    let resolution = if service.faults.resolve {
        Err(crate::journal::JournalError::InvalidInput(
            "injected outcome persistence failure".into(),
        ))
    } else {
        service.journal.resolve_intent(seq, outcome)
    };
    #[cfg(not(test))]
    let resolution = service.journal.resolve_intent(seq, outcome);
    if let Err(error) = resolution {
        result.status = TransactionStatus::Indeterminate;
        result.notes.push(format!(
            "live file states are reported, but durable intent resolution failed: {error}"
        ));
        if result.error.is_none() {
            result.error = Some(TransactionError::Journal(error.to_string()));
        }
    }
}
