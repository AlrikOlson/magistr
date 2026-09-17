use magistr_cockpit::cockpit::Model;

use crate::composer::Composer;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;

/// The composer at the foot of the turn column, composed from the model: the pill wears
/// the `Steer…` prompt while [`Model::prompt`] is empty and the typed text in the full
/// ink once it is not — the same predicate the cockpit's cell composer reads, so a
/// keystroke folded through the one `update` reshapes this composition with no text
/// state of its own.
pub struct ComposerComposition;

impl ComposerComposition {
    /// The pill — the one control here that takes focus.
    pub const PILL: ElementId = ElementId(150);

    /// The volume's conditional register glyph.
    const REGISTER: &'static str = "\u{203a}";
    /// The resting prompt, the design's own word.
    const STEER: &'static str = "Steer\u{2026}";
    /// The send/cancel caps the session pill carries at its right edge
    /// (view 05's own words).
    const HINT: &'static str = "\u{21b5} SEND \u{b7} \u{2318}. CANCEL \u{b7} ESC UP";

    /// The composer for `model`, in `medium`'s lexicon.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let text = model.prompt().text();
        if text.is_empty() {
            Composer::hinted(Self::REGISTER, Self::STEER, Self::HINT, Self::PILL, &lex)
        } else {
            Composer::hinted_filled(Self::REGISTER, text, Self::HINT, Self::PILL, &lex)
        }
    }
}

#[cfg(test)]
#[path = "comp_composer.tests.rs"]
mod tests;
