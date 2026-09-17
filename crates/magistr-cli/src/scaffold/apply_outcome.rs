use super::ApplyFailure;

/// What happened to one planned change when the plan was applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// The path was written, removed, or had its mode restored.
    Applied,
    /// The plan meant to leave the path alone, and did.
    Skipped {
        /// The plan's reason.
        reason: String,
    },
    /// The plan refused the path while planning; nothing was attempted.
    Refused {
        /// The plan's reason.
        reason: String,
    },
    /// The plan meant to apply the path and could not; the target is left
    /// as it was found.
    Failed(ApplyFailure),
}

impl ApplyOutcome {
    /// The lower-case label machine-readable output uses.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Skipped { .. } => "skipped",
            Self::Refused { .. } => "refused",
            Self::Failed(_) => "failed",
        }
    }

    /// Whether the change landed.
    #[must_use]
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied)
    }

    /// Whether the change was meant to land and did not.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// The reason or cause a reviewer sees beside a change that did not land.
    #[must_use]
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Applied => None,
            Self::Skipped { reason } | Self::Refused { reason } => Some(reason.clone()),
            Self::Failed(failure) => Some(failure.to_string()),
        }
    }
}
