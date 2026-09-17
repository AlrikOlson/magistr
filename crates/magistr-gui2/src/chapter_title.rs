use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A report's title in the serif display face — the volume's chapter heading (view 09):
/// `font-family: var(--serif)`, 24 px, −0.01 em, in the full ink.
///
/// The display face's one home. allows the serif on report titles and the project cover
/// at ≥20 px and nowhere else, so the face is not reachable through a [`TextRole`] or a
/// free [`Voice`] on an arbitrary element — a chapter's title is *this* component, and
/// the roster that says the serif appears in exactly one place is a list of its
/// callers.
///
/// [`TextRole`]: magistr_cockpit::layout::TextRole
pub struct ChapterTitle;

impl ChapterTitle {
    /// `title` set in [`Voice::DISPLAY`], wrapping at its measure.
    #[must_use]
    pub fn el(title: &str, lex: &Lexicon) -> Element {
        Element::text(Voice::DISPLAY, lex.tx, title).claim(Component::Title)
    }
}

#[cfg(test)]
#[path = "chapter_title.tests.rs"]
mod tests;
