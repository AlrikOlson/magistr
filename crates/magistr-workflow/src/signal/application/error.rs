/// A signal use-case failure, independent of transport error envelopes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    #[error("{0}")]
    InvalidArgs(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    InvalidState(String),
    #[error("signal_promote requires the roadmap engine (not wired in this context)")]
    NotWired,
    #[error("signal engine mutex poisoned")]
    Poisoned,
}

impl SignalError {
    /// Stable logical classification used by existing clients.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidArgs(_) => "invalid_args",
            Self::NotFound(_) => "not_found",
            Self::InvalidState(_) => "invalid_state",
            Self::NotWired => "not_wired",
            Self::Poisoned => "internal_error",
        }
    }
}
