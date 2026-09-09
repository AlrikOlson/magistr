use crate::element::Element;
use crate::lexicon::Lexicon;

/// A gate's light — the volume's `.gled`: a 9px rounded square that is either lit in
/// the success hue or dark on the second surface step. smallest component: a pass/fail
/// that would otherwise be a naked glyph. You read it; it can carry no id, no focus, no
/// pointer answer.
pub struct Led;

impl Led {
    /// `.gled { width:9px; height:9px; border-radius:2px }`.
    const SIDE: f32 = 9.0;
    const RADIUS: f32 = 2.0;

    /// One light: lit green when `on`, dark otherwise.
    #[must_use]
    pub fn el(on: bool, lex: &Lexicon) -> Element {
        Element::surface()
            .width(Self::SIDE)
            .height(Self::SIDE)
            .radius(Self::RADIUS)
            .fill(if on { lex.green } else { lex.s1 })
    }
}

#[cfg(test)]
#[path = "led.tests.rs"]
mod tests;
