use magistr_cockpit::cockpit::ShelfStill;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// One of a shipped chunk's stills on the Report's shelf (view 09): a
/// 16/10 tile at the card radius with the hairline ring the volume
/// gives it, the golden itself fitted inside — never scaled up, never
/// cropped — and the `still · <name>` cap in the tile's corner. The
/// volume's tile is a placeholder wash standing for a picture; this is
/// the picture.
pub struct StillTile;

impl StillTile {
    /// `aspect-ratio:16/10`.
    const ASPECT: f32 = 1.6;
    /// `border-radius:8px`.
    const RADIUS: f32 = 8.0;
    /// The cap sits `left:8px; bottom:6px` in the volume's own
    /// tracked uppercase, a size down (`font-size:8.5px`).
    const CAP_LEFT: f32 = 8.0;
    const CAP_BOTTOM: f32 = 6.0;
    const CAP: Voice = Voice {
        px: 8.5,
        ..Voice::CAP
    };

    /// The tile's height at `width` — the 16/10 box.
    #[must_use]
    pub fn height(width: f32) -> f32 {
        (width / Self::ASPECT).round()
    }

    /// The tile for `still`, `width` pixels wide.
    #[must_use]
    pub fn el(still: &ShelfStill, width: f32, lex: &Lexicon) -> Element {
        let height = Self::height(width);
        // Contain-fit inside the ring: the picture's height is the
        // tile's unless its aspect is wider than 16/10, in which case
        // the width decides — `Element::picture` sizes by height and
        // keeps the aspect, and never scales a small golden up.
        let (w, h) = still.size();
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (w as f32, h as f32);
        let (inner_w, inner_h) = (width - 2.0, height - 2.0);
        let fit = if h > 0.0 && w / h > inner_w / inner_h {
            inner_w * h / w
        } else {
            inner_h
        };
        Element::surface()
            .width(width)
            .height(height)
            .radius(Self::RADIUS)
            .fill(lex.mantle)
            .stroke(Lexicon::composite(lex.base, lex.hairc), 1.0)
            .center()
            .justify_center()
            .child(Element::picture(still.image.clone(), fit.max(1.0)))
            // The cap is two runs on one line: the kind word is the tile's own copy,
            // the golden's family is the file's name — a value, not a word the design
            // chose (sweep reads copy, and a file may be called anything).
            .child(
                Element::row()
                    .pin_left(Self::CAP_LEFT)
                    .pin_bottom(Self::CAP_BOTTOM)
                    .child(
                        Element::text(Self::CAP, lex.ov0, "still \u{b7} ")
                            .claim(Component::Cap)
                            .nowrap(),
                    )
                    .child(
                        Element::text(Self::CAP, lex.ov0, stem(&still.name))
                            .claim(Component::Value)
                            .nowrap(),
                    ),
            )
    }
}

/// The golden's family — `turn` of `turn-night.png`, `sky` of
/// `sky-rest-night.png`: the file stem up to its first dash, which is
/// the surface the golden suites name first and all a tile's corner
/// has room for.
fn stem(name: &str) -> &str {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.split('-').next().unwrap_or(stem)
}

#[cfg(test)]
#[path = "still_tile.tests.rs"]
mod tests;
