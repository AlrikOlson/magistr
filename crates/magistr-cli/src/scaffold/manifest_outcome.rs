use std::fmt;

/// What happened to the scaffold manifest while a plan was applied. The
/// manifest is the ownership evidence later plans judge by, so a save that
/// failed is reported beside the file outcomes rather than lost.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ManifestOutcome {
    /// No applied change needed a record.
    #[default]
    Untouched,
    /// Every record the applied changes needed was saved.
    Saved,
    /// The last save failed; records made after the previous successful
    /// save are not on disk.
    Failed {
        /// What went wrong.
        cause: String,
    },
}

impl ManifestOutcome {
    /// Whether the manifest failed to save.
    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

impl fmt::Display for ManifestOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Untouched => f.write_str("manifest untouched"),
            Self::Saved => f.write_str("manifest saved"),
            Self::Failed { cause } => write!(f, "manifest not saved: {cause}"),
        }
    }
}
