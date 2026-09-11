/// The retained statuses of the one background build a workspace can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexBuildState {
    Working,
    Completed,
    Failed,
}

impl IndexBuildState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Working)
    }
}
