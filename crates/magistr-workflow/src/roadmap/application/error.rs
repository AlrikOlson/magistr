/// A native roadmap failure with stable client-facing classification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoadmapError {
    #[error("{0}")]
    InvalidArgs(String),
    #[error("{0}")]
    InvalidState(String),
    #[error(
        "file export requires a configured workspace; use output: inline to read without writing"
    )]
    WorkspaceUnavailable,
    #[error("{0}")]
    ExportFailed(String),
    #[error("roadmap engine mutex poisoned")]
    Poisoned,
}

impl RoadmapError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidArgs(_) => "invalid_args",
            Self::InvalidState(_) => "invalid_state",
            Self::WorkspaceUnavailable => "workspace_unavailable",
            Self::ExportFailed(_) => "export_failed",
            Self::Poisoned => "internal_error",
        }
    }
}
