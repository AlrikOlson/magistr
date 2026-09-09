/// The caller's next kind of action, never an instruction to blindly retry.
/// Retry admission, deduplication and durable outcome lookup belong to adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    CorrectRequest,
    RefreshRevision,
    /// Review policy or configuration; this never turns a policy Deny into Ask.
    ReviewAuthorization,
    SelectAvailableCapability,
    RestorePersistence,
    ReconnectProvider,
    ReconcileOutcome,
}
