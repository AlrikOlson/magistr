/// A rejected plan or vote leaves the existing agreement unchanged.
#[derive(Debug, thiserror::Error)]
pub enum AgreementError {
    #[error("invalid peer plan: {0}")]
    Invalid(String),
    #[error("peer plan encoding: {0}")]
    Encoding(#[from] serde_json::Error),
    #[error("approval does not name the current project/run/revision/digest")]
    StaleApproval,
    #[error("voter is not a fixed peer session in this run")]
    UnknownPeer,
    #[error("peer plan revision counter exhausted")]
    RevisionOverflow,
    #[error("persisted plan does not match the trusted run contract")]
    ContractMismatch,
    #[error("persisted peer plan digest mismatch")]
    DigestMismatch,
}
