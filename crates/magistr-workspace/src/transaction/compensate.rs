use super::{
    FileDisposition, PreparedTransaction, TransactionOutcome, TransactionStatus,
    WorkspaceTransactions,
};
use std::path::{Path, PathBuf};

pub(super) fn compensate(
    service: &WorkspaceTransactions<'_>,
    prepared: &PreparedTransaction,
    attempted: &[bool],
    created: &[Vec<PathBuf>],
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
            &mut Vec::new(),
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
    #[cfg(test)]
    if let Some(hook) = &service.faults.before_directory_cleanup {
        hook();
    }
    remove_created_directories(&service.files.root, created, result);
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
/// Remove the directories this batch created, innermost first, only while
/// they are still empty. The non-recursive removal refuses a directory that
/// gained entries meanwhile, so a concurrent addition or an unrestored file
/// keeps its parent; the first such directory in a chain is named and the
/// chain's ancestors, which cannot be empty, are left without a second note.
fn remove_created_directories(
    root: &Path,
    created: &[Vec<PathBuf>],
    result: &mut TransactionOutcome,
) {
    for chain in created.iter().rev() {
        for directory in chain.iter().rev() {
            match std::fs::remove_dir(directory) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    // Workspace paths are `/`-separated on every platform.
                    let relative = directory
                        .strip_prefix(root)
                        .unwrap_or(directory)
                        .components()
                        .map(|component| component.as_os_str().to_string_lossy())
                        .collect::<Vec<_>>()
                        .join("/");
                    result.notes.push(format!(
                        "{relative}: compensation left a directory it created: {error}"
                    ));
                    break;
                }
            }
        }
    }
}
