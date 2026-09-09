//! One verb on a bad-day card.

use super::widget::Widget;

/// A verb a [`Fault`](super::Fault) offers: the raised button's word,
/// the widget the click folds as, and whether it is the card's one lit
/// exit (`docs/architecture/desktop.md#controls`) — the verb that *fixes* the fault,
/// as against the one that only tells more.
///
/// The widget is the verb's own id, so the click a card answers is the
/// same message the place it leads to would have sent — the Brief's
/// own contract ([`BriefBody::Act`](super::BriefBody::Act)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaultVerb {
    verb: &'static str,
    widget: Widget,
    lit: bool,
}

impl FaultVerb {
    /// A plain verb — a neutral button.
    #[must_use]
    pub const fn new(verb: &'static str, widget: Widget) -> Self {
        Self {
            verb,
            widget,
            lit: false,
        }
    }

    /// The lit verb — the card's one raised exit.
    #[must_use]
    pub const fn lit(verb: &'static str, widget: Widget) -> Self {
        Self {
            verb,
            widget,
            lit: true,
        }
    }

    /// The button's word — a verb, never a noun.
    #[must_use]
    pub const fn verb(&self) -> &'static str {
        self.verb
    }

    /// The widget the click folds as.
    #[must_use]
    pub const fn widget(&self) -> Widget {
        self.widget
    }

    /// Whether this is the card's lit exit.
    #[must_use]
    pub const fn is_lit(&self) -> bool {
        self.lit
    }
}
