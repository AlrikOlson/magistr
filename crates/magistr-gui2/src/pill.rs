use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// data class: a flat mono pill you *read* — `.chip7` in the volume. Never
/// clickable-looking, and structurally never clickable: nothing here can attach an
/// [`ElementId`](crate::ElementId), take focus, or answer a pointer, which is the
/// class's law carried in the type rather than in a review.
///
/// Three faces, the volume's own: the **live** chip (`.chip7.acc`, mauve, a status
/// dot), the **ok** chip (`.chip7.ok`, green), and the **numeric** chip (`.chip7` with
/// `.num`, subdued ink) — each a 1px ring at the token's own alpha over the transparent
/// chip.
pub struct DataPill;

impl DataPill {
    /// The chip's CSS padding, plus the 1px border the border-box
    /// includes (`padding:3px 11px; border:1px`).
    const PAD_X: f32 = 12.0;
    const PAD_Y: f32 = 4.0;
    /// `border-radius:999px` — the stadium; the painter caps it at
    /// half the box.
    const RADIUS: f32 = 999.0;
    /// The share of a glow token the chip's ring keeps
    /// (`color-mix(in oklab, var(--glow-*) 45%, transparent)`).
    const RING_ALPHA: f32 = 0.45;
    /// The status dot's diameter (`.dot7`).
    const DOT: f32 = 6.0;
    /// The gap between the dot and the copy (`.chip7 { gap:6px }`).
    const GAP: f32 = 6.0;

    /// The live chip: mauve ink, mauve ring, a status dot —
    /// `Executing · 04:12`. The translucent ring is precomposed onto
    /// the base in sRGB — see [`Lexicon::composite`] for why.
    #[must_use]
    pub fn live(copy: &str, lex: &Lexicon) -> Element {
        let ring = lex.grounded(faint(lex.glow_m, Self::RING_ALPHA));
        Self::chip(copy, lex.mauve, ring, Some(lex.mauve))
    }

    /// The ok chip: green ink and ring — `gates ✓✓✓`.
    #[must_use]
    pub fn ok(copy: &str, lex: &Lexicon) -> Element {
        let ring = lex.grounded(faint(lex.glow_g, Self::RING_ALPHA));
        Self::chip(copy, lex.green, ring, None)
    }

    /// The numeric chip: subdued ink, the hairline ring —
    /// `1.84 / 12.00`.
    #[must_use]
    pub fn num(copy: &str, lex: &Lexicon) -> Element {
        Self::chip(copy, lex.sb0, lex.grounded(lex.hairc), None)
    }

    /// The quiet status chip: the plain `.chip7` with its `.dot7` in
    /// the subdued ink — the Galaxy's `1 action required`, which the
    /// volume draws as a count and not as an alarm (the amber is the
    /// cluster's).
    #[must_use]
    pub fn quiet(copy: &str, lex: &Lexicon) -> Element {
        Self::chip(copy, lex.sb0, lex.grounded(lex.hairc), Some(lex.sb0))
    }

    /// The label chip: the plain `.chip7` in the subdued ink with no
    /// dot — the identity band's `sonnet · Draft mode`, a name worn
    /// beside the chunk chip and read, never pressed.
    #[must_use]
    pub fn label(copy: &str, lex: &Lexicon) -> Element {
        Self::chip(copy, lex.sb0, lex.grounded(lex.hairc), None)
    }

    /// The marked chip: the plain `.chip7` in the subdued ink with its
    /// `.dot7` in a state's hue — the Signals header's `2 open` with the
    /// peach dot (view 12): a count wearing the state it counts, still
    /// read and never pressed.
    #[must_use]
    pub fn marked(copy: &str, dot: Srgba, lex: &Lexicon) -> Element {
        Self::chip(copy, lex.sb0, lex.grounded(lex.hairc), Some(dot))
    }

    /// The alarm chip: the `.chip7` worn in a state's hue — ink, ring
    /// at the ring alpha, and the dot — the bad day's `● 2 blocking`
    /// (view 14). A count in the hue of what it counts, still read and
    /// never pressed.
    #[must_use]
    pub fn alarm(copy: &str, hue: Srgba, lex: &Lexicon) -> Element {
        let ring = lex.grounded(faint(hue, Self::RING_ALPHA));
        Self::chip(copy, hue, ring, Some(hue))
    }

    /// The tag: the plain chip at the small size a step card wears its
    /// execution refs in (view 11's `.chip7` at `font-size:9px;
    /// padding:1px 8px`, plus the border the box includes).
    #[must_use]
    pub fn tag(copy: &str, lex: &Lexicon) -> Element {
        Self::sized(
            copy,
            lex.sb0,
            lex.grounded(lex.hairc),
            None,
            Self::TAG,
            (Self::TAG_PAD_X, Self::TAG_PAD_Y),
        )
    }

    /// The tag's voice — the chip's mono at 9 — and its border-box
    /// paddings.
    const TAG: Voice = Voice {
        family: "mono",
        px: 9.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    const TAG_PAD_X: f32 = 9.0;
    const TAG_PAD_Y: f32 = 2.0;

    /// One chip: a stadium ring around a single unbreakable run, with
    /// an optional status dot in the ink's own color.
    fn chip(copy: &str, ink: Srgba, ring: Srgba, dot: Option<Srgba>) -> Element {
        Self::sized(
            copy,
            ink,
            ring,
            dot,
            Voice::CHIP,
            (Self::PAD_X, Self::PAD_Y),
        )
    }

    /// The chip at a given voice and padding.
    fn sized(
        copy: &str,
        ink: Srgba,
        ring: Srgba,
        dot: Option<Srgba>,
        voice: Voice,
        pad: (f32, f32),
    ) -> Element {
        let mut chip = Element::row()
            .pad_x(pad.0)
            .pad_y(pad.1)
            .gap(Self::GAP)
            .center()
            .radius(Self::RADIUS)
            .stroke(ring, 1.0);
        if let Some(dot) = dot {
            chip = chip.child(
                Element::surface()
                    .width(Self::DOT)
                    .height(Self::DOT)
                    .radius(Self::DOT / 2.0)
                    .fill(dot),
            );
        }
        chip.child(
            Element::text(voice, ink, copy)
                .claim(Component::Chip)
                .nowrap(),
        )
    }
}

/// `color` at `alpha` — CSS's mix with transparent.
fn faint(color: Srgba, alpha: f32) -> Srgba {
    Srgba::new(color.r, color.g, color.b, alpha)
}

#[cfg(test)]
#[path = "pill.tests.rs"]
mod tests;
