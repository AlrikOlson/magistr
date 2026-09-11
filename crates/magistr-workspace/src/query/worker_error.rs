use crate::{
    contracts::{ErrorKind, WorkspaceError},
    worker::WorkerError,
};

pub(super) fn worker_error(error: WorkerError) -> WorkspaceError {
    let kind = match &error {
        WorkerError::Busy => ErrorKind::EngineBusy { backoff_ms: 50 },
        WorkerError::Quarantined(_) => ErrorKind::InputQuarantined,
        WorkerError::UnsupportedVersion => ErrorKind::UnsupportedVersion,
        WorkerError::Remote { code, .. } if code == "PATTERN_INVALID" => ErrorKind::PatternInvalid,
        _ => ErrorKind::EngineUnavailable,
    };
    WorkspaceError::new(kind, error.to_string())
}
