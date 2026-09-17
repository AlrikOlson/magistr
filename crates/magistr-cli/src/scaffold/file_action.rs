use super::{Intent, Refusal};

/// What an administrative plan does to one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAction {
    /// File does not exist — would be created.
    Create,
    /// File exists and content differs — would be overwritten.
    Overwrite,
    /// File exists and the plan removes it.
    Remove,
    /// File exists with identical content — no change needed.
    Unchanged,
    /// File exists and its ownership protects the current content — skipped.
    Preserved,
    /// The plan will not touch this path, and says why.
    Refused(Refusal),
}

impl FileAction {
    /// Whether this action is applied, skipped or refused.
    #[must_use]
    pub fn intent(&self) -> Intent {
        match self {
            Self::Create | Self::Overwrite | Self::Remove => Intent::Apply,
            Self::Unchanged | Self::Preserved => Intent::Skip,
            Self::Refused(_) => Intent::Refuse,
        }
    }

    /// The lower-case label machine-readable output uses.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Overwrite => "overwrite",
            Self::Remove => "remove",
            Self::Unchanged => "unchanged",
            Self::Preserved => "preserved",
            Self::Refused(_) => "refused",
        }
    }

    /// Whether the action writes or removes the file.
    #[must_use]
    pub fn is_applied(&self) -> bool {
        self.intent() == Intent::Apply
    }
}
