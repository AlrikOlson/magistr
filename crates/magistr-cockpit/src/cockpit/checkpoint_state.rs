//! Where a checkpoint got to.

/// A row on the spine is a promise before it is a fact.
///
/// The permission card names its checkpoint in the same fold that
/// raises the card, which is *before* the port has taken it — so the
/// rail has a row for a state that may not exist yet. Rather than
/// hide the row until it is real (which would leave the card naming
/// nothing on screen) or show it as if it were (which would put an
/// unreachable state on a chain of verified ones), the row says which
/// of the three it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointState {
    /// Asked for; the port has not answered.
    Saving,
    /// Taken.
    Saved {
        /// The commit it can be forked from, when it was taken in a
        /// repository.
        head: Option<String>,
        /// Whether that commit describes the tree — a clean checkpoint
        /// a fork can actually reproduce.
        verified: bool,
    },
    /// The port refused. The row stays, because the card that named it
    /// is on screen: a name that turned out to be nothing is worth more
    /// to the reader than a name that vanished.
    Failed(String),
}

impl CheckpointState {
    /// The state in the words the rail prints after the reason.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Saving => "saving",
            Self::Saved { verified: true, .. } => "verified",
            Self::Saved { .. } => "unverified",
            Self::Failed(_) => "not saved",
        }
    }
}
