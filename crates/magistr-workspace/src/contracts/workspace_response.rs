use super::{Budget, EnvelopeVersion, Freshness, ResultSet, WorkspaceError};

/// A typed answer. Success data and failure payload cannot coexist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceResponse<R> {
    Success {
        version: EnvelopeVersion,
        freshness: Freshness,
        budget: Budget,
        results: ResultSet<R>,
        notes: Option<Vec<String>>,
    },
    Failure {
        version: EnvelopeVersion,
        error: WorkspaceError,
        notes: Option<Vec<String>>,
    },
}
