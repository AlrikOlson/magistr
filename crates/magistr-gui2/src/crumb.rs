use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::crumb_stop::CrumbStop;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::voice::Voice;

/// Navigation: the crumb trail — `.crumb` in the volume. Each stop names a
/// place (`projects ▸ magistr`); the stops behind you answer a click, the one you are
/// at is emphasized and inert.
pub struct Crumb;

impl Crumb {
    /// The gap between stops and separators (`.crumb { gap:7px }`).
    const GAP: f32 = 7.0;
    /// The gap between a stop's mark and its label (`.crumb .c`).
    const MARK_GAP: f32 = 5.0;
    /// An icon's box (`.ic` — 13px); a sigil's (`.sig` — 14px).
    const ICON_PX: u32 = 13;
    const SIGIL_PX: u32 = 14;
    /// The separator's own size (`.sep7 { font-size:9px }`).
    const SEP_PX: f32 = 9.0;

    /// The trail, separators laid between the stops.
    #[must_use]
    pub fn el(stops: &[CrumbStop], lex: &Lexicon) -> Element {
        let sep_voice = Voice {
            px: Self::SEP_PX,
            ..Voice::CRUMB
        };
        let mut row = Element::row().gap(Self::GAP).center();
        for (index, stop) in stops.iter().enumerate() {
            if index > 0 {
                row = row.child(
                    Element::text(sep_voice, lex.s1, "\u{25b8}")
                        .claim(Component::Crumb)
                        .nowrap(),
                );
            }
            row = row.child(Self::stop(stop, lex));
        }
        row
    }

    /// One stop: its mark in its own ink, then its label. A `here`
    /// stop wears full ink and no id; a back stop is subdued and
    /// answers the pointer.
    fn stop(stop: &CrumbStop, lex: &Lexicon) -> Element {
        let ink = if stop.here { lex.tx } else { lex.ov0 };
        let voice = Voice {
            weight: if stop.here { 600 } else { Voice::CRUMB.weight },
            ..Voice::CRUMB
        };
        let mut el = Element::row().gap(Self::MARK_GAP).center();
        if let Some(mark) = stop.mark {
            let px = match mark {
                Mark::Sigil(_) => Self::SIGIL_PX,
                Mark::Icon(_) => Self::ICON_PX,
            };
            el = el.child(Element::mark(mark, px, bytes(ink)));
        }
        el = el.child(
            Element::text(voice, ink, stop.label.as_str())
                .claim(Component::Crumb)
                .nowrap(),
        );
        if let Some(id) = stop.id {
            el = el
                .id(id)
                .focusable()
                .focus_ring(lex.mauve)
                .hover_fill(lex.mantle)
                .press_fill(lex.deep);
        }
        el
    }
}

/// A color's bytes, the form a mark's raster is keyed by.
fn bytes(c: Srgba) -> (u8, u8, u8) {
    let b = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    (b(c.r), b(c.g), b(c.b))
}

#[cfg(test)]
#[path = "crumb.tests.rs"]
mod tests;
