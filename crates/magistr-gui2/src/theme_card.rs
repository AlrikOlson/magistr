use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::theme_preview::ThemePreview;
use crate::voice::Voice;

/// Selection, for visual choices: the theme card — the volume's `.tcard`, a
/// live media preview over its label, chosen by ring-and-check rather than by a filled
/// ground (you cannot fill a picture).
pub struct ThemeCard;

impl ThemeCard {
    /// The preview's box in the settings sheet (`.tp` inline 96×54)
    /// and its radius.
    pub const PREVIEW_W: f32 = 96.0;
    pub const PREVIEW_H: f32 = 54.0;
    const RADIUS: f32 = 9.0;
    /// The gap between preview and label (`.tcard { gap:7px }`), and
    /// inside the label row (`.tl2 { gap:6px }`).
    const GAP: f32 = 7.0;
    /// The chosen badge (`.tchk` — 14px circle, 6px off the corner)
    /// and its check mark (9px, the volume's own night-base ink).
    const BADGE: f32 = 14.0;
    const BADGE_INSET: f32 = 6.0;
    const CHECK_PX: u32 = 9;

    /// The card at the specimen sheet's measure: `preview` (a 96×54
    /// element carrying its own media) under this card's ring, `label`
    /// beneath, the check badge when `chosen`. Answers on `id` — a
    /// visual choice is interactive.
    #[must_use]
    pub fn el(
        label: &str,
        chosen: bool,
        preview: Element,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        Self::sized(
            label,
            chosen,
            preview,
            (Self::PREVIEW_W, Self::PREVIEW_H),
            id,
            lex,
        )
    }

    /// The card with its preview at `size` — view 10's settings pane
    /// shows the volume's 132×74 drawing where the sheet shows 96×54.
    #[must_use]
    pub fn sized(
        label: &str,
        chosen: bool,
        preview: Element,
        size: (f32, f32),
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        // The ring: inset hairline at rest, the 2px accent when chosen
        // (`box-shadow: inset 0 0 0 1px var(--hairc)` / `2px mauve`).
        // An inset box-shadow paints over the background and under
        // nothing else, and the media is the background — so the ring
        // is a child over the media, not the preview's own stroke,
        // which the media's strips would otherwise cover.
        let (ring, ring_width) = if chosen {
            (lex.mauve, 2.0)
        } else {
            (lex.hairc, 1.0)
        };
        let mut preview = preview
            .width(size.0)
            .height(size.1)
            .radius(Self::RADIUS)
            .child(
                Element::surface()
                    .pin_left(0.0)
                    .pin_top(0.0)
                    .pin_right(0.0)
                    .pin_bottom(0.0)
                    .radius(Self::RADIUS)
                    .stroke(ring, ring_width),
            );
        if chosen {
            preview = preview.child(
                Element::surface()
                    .width(Self::BADGE)
                    .height(Self::BADGE)
                    .radius(Self::BADGE / 2.0)
                    .fill(lex.mauve)
                    .pin_top(Self::BADGE_INSET)
                    .pin_right(Self::BADGE_INSET)
                    .center()
                    .justify_center()
                    // .tchk svg { stroke:#161622 } — the night base, in
                    // both media: the badge is always the accent disc.
                    .child(Element::mark(
                        Mark::Icon(Icon::Done),
                        Self::CHECK_PX,
                        (0x16, 0x16, 0x22),
                    )),
            );
        }
        let (ink, weight) = if chosen {
            (lex.tx, 550)
        } else {
            (lex.sb1, Voice::THEME_LABEL.weight)
        };
        let voice = Voice {
            weight,
            ..Voice::THEME_LABEL
        };
        Element::surface()
            .gap(Self::GAP)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve)
            .child(preview)
            .child(
                Element::text(voice, ink, label)
                    .claim(Component::ThemeCard)
                    .nowrap(),
            )
    }

    /// The night preview the specimen sheet shows for "The sky": the
    /// deep night ground with two stars. The literals are the sheet's
    /// own inline SVG (`background:#14141f`, circles in green and
    /// mauve) — media content, not palette.
    #[must_use]
    pub fn sky_preview(lex: &Lexicon) -> Element {
        let ground = Srgba::new(
            0x14 as f32 / 255.0,
            0x14 as f32 / 255.0,
            0x1f as f32 / 255.0,
            1.0,
        );
        Element::surface()
            .fill(ground)
            .child(ThemePreview::dot(30.0, 30.0, 2.0, lex.green))
            .child(ThemePreview::dot(62.0, 22.0, 2.6, lex.mauve))
    }
}

#[cfg(test)]
#[path = "theme_card.tests.rs"]
mod tests;
