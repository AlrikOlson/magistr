/// Whether a planned change is applied, skipped or refused when the plan
/// is executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// The path is written or removed.
    Apply,
    /// The path is left as it is because nothing needs to change or its
    /// owner keeps it.
    Skip,
    /// The path is left as it is because the plan cannot act on it truthfully.
    Refuse,
}

impl Intent {
    /// The lower-case label machine-readable output uses.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Skip => "skip",
            Self::Refuse => "refuse",
        }
    }
}
