//! One thing that happened while you were away.

use super::brief_body::BriefBody;
use crate::theme::Severity;

/// A catch-up card on the Brief: its severity band, the cap that names
/// the kind and counts it, and the body.
///
/// The band is the card's whole colour: `docs/architecture/desktop.md#themes` draws
/// severity as a tinted ground plus a coloured icon cap, so a card
/// carries a [`Severity`] and the pen answers the pair. There is
/// deliberately nowhere here to put a border colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefCard {
    severity: Severity,
    cap: String,
    body: BriefBody,
}

impl BriefCard {
    /// A card in `severity`'s band, capped `cap`, carrying `body`.
    ///
    /// `cap` is written in the case the copy laws write it in — the
    /// [`Cap`](crate::layout::TextRole::Cap) voice is what uppercases
    /// it, so a card that shouted in its own string would shout twice
    /// on a window and once on a surface with no voices at all.
    #[must_use]
    pub fn new(severity: Severity, cap: impl Into<String>, body: BriefBody) -> Self {
        Self {
            severity,
            cap: cap.into(),
            body,
        }
    }

    /// The card's band.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// The cap — `Stuck · 4 h`, `Done · 2`, `Spend · 3 sessions`.
    #[must_use]
    pub fn cap(&self) -> &str {
        &self.cap
    }

    /// The body.
    #[must_use]
    pub const fn body(&self) -> &BriefBody {
        &self.body
    }
}

#[cfg(test)]
#[path = "brief_card.tests.rs"]
mod tests;
