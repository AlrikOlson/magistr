use super::{
    WorkspaceCommands, WriteInput, WriteReport,
    gate::{self, GateInput},
    resolved::ResolvedWrites,
    source_reads::SourceReads,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError, WorkspaceResponse},
    file_content::FileContent,
    query::budget,
    transaction::{ExpectedState, FormatMode, TransactionChange},
};

/// Resolve a whole-file write into one anchored change and run it through
/// the shared pipeline: the current bytes (or their absence) are the
/// anchor, the caller's bytes are the content, formatting is off so what
/// lands is what was written, and the caller's authorization stands in for
/// the confirm gate.
pub(super) async fn run(
    service: &mut WorkspaceCommands<'_>,
    input: &WriteInput,
) -> WorkspaceResponse<WriteReport> {
    let path = SourceReads::normalize(&input.path);
    if path.is_empty() {
        return budget::failure(WorkspaceError::new(
            ErrorKind::PatternInvalid,
            "write: requires path",
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
    let current = match service.reads.read(&path) {
        Ok(current) => current,
        Err(error) => return budget::failure(error),
    };
    if let Some(expected) = &input.expected_hash
        && current.as_ref().map(FileContent::content_hash).as_ref() != Some(expected)
    {
        return budget::failure(WorkspaceError::new(
            ErrorKind::StaleAnchor {
                content_hash: service.reads.current_hash(&path),
            },
            format!(
                "{path:?} changed since it was read (expectedHash mismatch); re-read the file and re-derive the write against the returned contentHash"
            ),
        ));
    }
    let expected = current.as_ref().map_or(ExpectedState::Missing, |content| {
        ExpectedState::Hash(content.content_hash())
    });
    let resolved = ResolvedWrites {
        changes: vec![TransactionChange {
            path: path.clone(),
            expected,
            content: Some(FileContent::decode(input.content.clone().into_bytes())),
            baseline: None,
        }],
        notes: vec![format!(
            "write {path:?}: exact bytes through the pipeline, journaled"
        )],
        always_gated: false,
    };
    gate::run(
        service,
        GateInput {
            operation: "write",
            intent,
            confirm: None,
            dry_run: false,
            auto_apply: true,
            budget: input.budget,
            format: FormatMode::Disabled,
        },
        resolved,
    )
    .await
}
