//! What one catch-up card puts under its cap.

use super::widget::Widget;

/// The data shown by a catch-up card.
///
/// Actionable cards carry one primary action. Spend is a meter with no action;
/// other cards retain the fields needed to navigate to the relevant work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BriefBody {
    /// A sentence and the one raised verb that acts on it. The widget
    /// is the verb's own id, so the click the card answers is the same
    /// message the place it dives to would have sent.
    Act {
        /// The sentence, in the copy laws' STE.
        text: String,
        /// The verb on the raised button — a verb, never a noun.
        verb: &'static str,
        /// The button's id.
        widget: Widget,
    },
    /// A quantity against its cap: the readout at the card's far edge
    /// and the meter under it, in per-mille of the cap so the geometry
    /// stays `Eq` — [`Control::Meter`](crate::layout::Control)'s own
    /// units.
    Measure {
        /// The readout — `3.61 / 12.00 USD`.
        readout: String,
        /// How full the meter is, in per-mille of its cap.
        fill: u16,
        /// Where the breaker's tick sits, in per-mille, when one binds.
        tick: Option<u16>,
    },
}

impl BriefBody {
    /// The verb this card offers, when it offers one.
    #[must_use]
    pub const fn verb(&self) -> Option<(&str, Widget)> {
        match self {
            Self::Act { verb, widget, .. } => Some((*verb, *widget)),
            Self::Measure { .. } => None,
        }
    }
}

#[cfg(test)]
#[path = "brief_body.tests.rs"]
mod tests;
