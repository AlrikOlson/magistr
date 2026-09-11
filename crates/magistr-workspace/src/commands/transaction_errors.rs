use super::{diff::unified_diff, source_reads::SourceReads};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    query::worker_error,
    transaction::{TransactionChange, TransactionError},
};

/// Map a pipeline refusal onto the retained taxonomy with its required payload.
pub(super) fn map(
    reads: &SourceReads<'_>,
    error: TransactionError,
    changes: &[TransactionChange],
) -> WorkspaceError {
    match error {
        TransactionError::Policy(error) => error,
        TransactionError::Worker(error) => worker_error(error),
        TransactionError::StaleAnchor(path) => {
            let file = path.split(": ").next().unwrap_or(&path).to_owned();
            WorkspaceError::new(
                ErrorKind::StaleAnchor {
                    content_hash: reads.current_hash(&file),
                },
                format!(
                    "{path}: the file changed since it was read (expectedHash mismatch); re-read it and re-derive the edit against the returned contentHash"
                ),
            )
        }
        TransactionError::ParseRejected { path, message } => {
            let proposed = changes.iter().find(|change| change.path == path);
            let before = reads.read(&path).ok().flatten();
            let rejected_diff = proposed.map_or_else(String::new, |change| {
                unified_diff(&path, before.as_ref(), change.content.as_ref())
            });
            WorkspaceError::new(
                ErrorKind::ParseRejected {
                    parser_message: message.clone(),
                    rejected_diff,
                },
                format!(
                    "{path}: the write would introduce a syntax error ({message}); nothing was applied"
                ),
            )
        }
        TransactionError::Confirmation(message) => {
            WorkspaceError::new(ErrorKind::ConfirmMismatch, message)
        }
        TransactionError::InvalidInput(message) => {
            WorkspaceError::new(ErrorKind::PatternInvalid, message)
        }
        TransactionError::Io(message) | TransactionError::Journal(message) => {
            WorkspaceError::new(ErrorKind::EngineUnavailable, message)
        }
    }
}
