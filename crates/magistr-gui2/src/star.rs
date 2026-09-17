use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::lexicon::Lexicon;

/// A roadmap star with six states and workstream colors. Geometry matches the
/// reference SVG: a ring's box is `2r + stroke`, aligning its inset stroke with the
/// SVG's centered stroke. The sky composition handles interaction.
pub struct Star;

impl Star {
    /// The ring rhythm — the volume's `stroke-dasharray="3 3"`.
    const DASH: f32 = 3.0;

    /// Pending: a faint dashed ring, r 4.5 at stroke 1.
    #[must_use]
    pub fn pending(lex: &Lexicon) -> Element {
        Self::ring(4.5, 1.0, lex.ov0, true)
    }

    /// Ready: the accent dot inside its standing ring (r 7 at stroke
    /// 1.5, the ring at the pulse's rest opacity).
    #[must_use]
    pub fn ready(lex: &Lexicon) -> Element {
        let ring = lex.grounded(Srgba::new(lex.mauve.r, lex.mauve.g, lex.mauve.b, 0.8));
        Self::with_core(Self::ring(7.0, 1.5, ring, false), 2.5, lex.mauve)
    }

    /// Executing: the accent dot at its full r 5.
    #[must_use]
    pub fn executing(lex: &Lexicon) -> Element {
        Self::dot(5.0, lex.mauve)
    }

    /// Stuck: the attention dot, r 4.5 — needs a decision.
    #[must_use]
    pub fn stuck(lex: &Lexicon) -> Element {
        Self::dot(4.5, lex.peach)
    }

    /// Blocked: a danger dashed ring (r 7.5 at stroke 1) around the
    /// danger dot — cannot start.
    #[must_use]
    pub fn blocked(lex: &Lexicon) -> Element {
        Self::with_core(Self::ring(7.5, 1.0, lex.red, true), 2.5, lex.red)
    }

    /// Done: the settled success dot, r 3.
    #[must_use]
    pub fn done(lex: &Lexicon) -> Element {
        Self::dot(3.0, lex.green)
    }

    /// A workstream's hue dot, r 4 — the constellation's legend.
    #[must_use]
    pub fn hue(color: Srgba) -> Element {
        Self::dot(4.0, color)
    }

    /// A filled circle of radius `r` — SVG geometry in the volume, so
    /// painted where it falls rather than snapped to the grid
    /// ([`Element::exact`]).
    fn dot(r: f32, color: Srgba) -> Element {
        Element::surface()
            .width(r * 2.0)
            .height(r * 2.0)
            .radius(r)
            .fill(color)
            .exact()
    }

    /// A circular ring: the box is `2r + width` so the inset stroke
    /// covers `r ± width/2`, exactly the SVG's centered stroke — and,
    /// like the SVG's, drawn at its fractional centre, never snapped.
    fn ring(r: f32, width: f32, color: Srgba, dashed: bool) -> Element {
        let side = r * 2.0 + width;
        let mut ring = Element::surface()
            .width(side)
            .height(side)
            .radius(side / 2.0)
            .stroke(color, width)
            .exact();
        if dashed {
            ring = ring.dashed().dash(Self::DASH, Self::DASH);
        }
        ring
    }

    /// `ring` with a core dot of radius `r` pinned at its center.
    fn with_core(ring: Element, r: f32, color: Srgba) -> Element {
        let side = ring.width.unwrap_or(0.0);
        let inset = (side - r * 2.0) / 2.0;
        ring.child(Self::dot(r, color).pin_left(inset).pin_top(inset))
    }
}

#[cfg(test)]
#[path = "star.tests.rs"]
mod tests;
