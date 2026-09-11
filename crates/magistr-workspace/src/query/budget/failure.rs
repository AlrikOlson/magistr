use crate::contracts::{EnvelopeVersion, WorkspaceError, WorkspaceResponse};

pub(in crate::query) fn failure<R>(error: WorkspaceError) -> WorkspaceResponse<R> {
    WorkspaceResponse::Failure {
        version: EnvelopeVersion::LATEST,
        error,
        notes: None,
    }
}
