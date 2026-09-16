//! How a recorded session ended.

use crate::pixel::Icon;
use crate::theme::Icons;

/// The outcome of a past session, as the launcher's sessions card shows
/// it — a fact about the session, drawn as a glyph by the view so the
/// mark follows the icon vocabulary in force rather than being baked in
/// by the entry point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionOutcome {
    /// The session finished.
    #[default]
    Success,
    /// The session ended in an error.
    Error,
    /// The session was cancelled.
    Cancelled,
}

impl SessionOutcome {
    /// The mark for this outcome.
    #[must_use]
    pub const fn icon(self) -> Icon {
        match self {
            Self::Success => Icon::Done,
            Self::Error => Icon::Failed,
            Self::Cancelled => Icon::Waiting,
        }
    }

    /// The one-cell mark for this outcome in `icons`' vocabulary.
    #[must_use]
    pub const fn glyph(self, icons: &Icons) -> &'static str {
        icons.of(self.icon())
    }
}
