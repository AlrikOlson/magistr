use crate::contracts::{EnvelopeVersion, WorkspaceError, WorkspaceResponse};

pub(crate) fn failure<R>(error: WorkspaceError) -> WorkspaceResponse<R> {
    WorkspaceResponse::Failure {
        version: EnvelopeVersion::LATEST,
        error,
        notes: None,
    }
}
