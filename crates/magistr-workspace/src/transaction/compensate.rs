use super::{
    FileDisposition, PreparedTransaction, TransactionOutcome, TransactionStatus,
    WorkspaceTransactions,
};

pub(super) fn compensate(
    service: &WorkspaceTransactions<'_>,
    prepared: &PreparedTransaction,
    attempted: &[bool],
    result: &mut TransactionOutcome,
) {
    let mut observed_applied = vec![false; prepared.preview.len()];
    for index in (0..prepared.preview.len()).rev() {
        if !attempted[index] {
            continue;
        }
        let change = &prepared.request.changes[index];
        let snapshot = &prepared.snapshots[index];
        let path = &prepared.paths[index];
        match service.files.snapshot(service.policy, &change.path, path) {
            Ok(current) if current.same(snapshot) => continue,
            Ok(current) if current.content == prepared.preview[index].after => {
                // A publish can succeed before a later sync error. Remember the
                // observed effect independently of either replacement's result.
                observed_applied[index] = true;
            }
            Ok(_) => {
                result.notes.push(format!(
                    "{}: compensation refused to overwrite an unexpected third state",
                    change.path
                ));
                continue;
            }
            Err(error) => {
                result.notes.push(format!(
                    "{}: compensation could not inspect current state: {error}",
                    change.path
                ));
                continue;
            }
        }
        #[cfg(test)]
        if service.faults.compensation_at == Some(index) {
            result
                .notes
                .push(format!("{}: injected compensation failure", change.path));
            continue;
        }
        let replacement = service.files.replace(
            service.policy,
            &change.path,
            path,
            snapshot.content.as_ref(),
            snapshot.permissions.as_ref(),
        );
        #[cfg(test)]
        let replacement = replacement.and_then(|()| {
            if service.faults.after_compensation_at == Some(index) {
                Err(super::TransactionError::Io(
                    "injected post-rename compensation failure".into(),
                ))
            } else {
                Ok(())
            }
        });
        if let Err(error) = replacement {
            result.notes.push(format!(
                "{}: compensation effect failed: {error}",
                change.path
            ));
        }
    }
    result.status = TransactionStatus::Failed;
    // Re-observe every target, including a failed rename and untouched siblings.
    // Never infer source state from the journal or from a compensation return code.
    for (index, file) in result.files.iter_mut().enumerate() {
        let change = &prepared.request.changes[index];
        match service
            .files
            .snapshot(service.policy, &change.path, &prepared.paths[index])
        {
            Ok(current) if current.same(&prepared.snapshots[index]) => {
                file.disposition = if file.preview.before == file.preview.after {
                    FileDisposition::Unchanged
                } else if observed_applied[index] || file.disposition == FileDisposition::Applied {
                    FileDisposition::RolledBack
                } else {
                    FileDisposition::NotApplied
                };
            }
            Ok(current) if current.content == file.preview.after => {
                file.disposition = FileDisposition::Applied;
                result.status = TransactionStatus::Indeterminate;
            }
            Ok(_) | Err(_) => {
                file.disposition = FileDisposition::Indeterminate;
                result.status = TransactionStatus::Indeterminate;
            }
        }
    }
}
