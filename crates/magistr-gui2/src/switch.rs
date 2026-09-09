use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;

/// Selection, for booleans: the switch — the volume's `.tgl`, a 32×19
/// stadium whose thumb sits left when off and right when on, the track wearing the
/// accent only while on.
pub struct Switch;

impl Switch {
    /// The track (`.tgl { width:32px; height:19px }`).
    pub const WIDTH: f32 = 32.0;
    pub const HEIGHT: f32 = 19.0;
    /// The thumb (`.tgl i { width:14px; top:2.5px; left:2.5px }`).
    const THUMB: f32 = 14.0;
    const INSET: f32 = 2.5;

    /// The switch, answering on `id`.
    #[must_use]
    pub fn el(on: bool, id: ElementId, lex: &Lexicon) -> Element {
        let track = if on { lex.mauve } else { lex.s1 };
        // `.tgl i { box-shadow:0 1px 2px rgba(0,0,0,.3) }` — drawn as
        // a disc one pixel below the thumb, precomposed onto the track
        // (a glow primitive would be Enhanced-only and invisible on
        // the CPU tier, which is what the parity gate photographs).
        let shadow = Element::surface()
            .width(Self::THUMB)
            .height(Self::THUMB)
            .radius(Self::THUMB / 2.0)
            .fill(Lexicon::composite(
                track,
                qs_gpu::color::Srgba::new(0.0, 0.0, 0.0, 0.3),
            ))
            .pin_top(Self::INSET + 1.0);
        let thumb = Element::surface()
            .width(Self::THUMB)
            .height(Self::THUMB)
            .radius(Self::THUMB / 2.0)
            .fill(lex.base)
            .pin_top(Self::INSET);
        let (shadow, thumb) = if on {
            (shadow.pin_right(Self::INSET), thumb.pin_right(Self::INSET))
        } else {
            (shadow.pin_left(Self::INSET), thumb.pin_left(Self::INSET))
        };
        Element::row()
            .width(Self::WIDTH)
            .height(Self::HEIGHT)
            .radius(999.0)
            .fill(track)
            .hover_fill(lift(track))
            .id(id)
            .focusable()
            .focus_ring(lex.mauve)
            .child(shadow)
            .child(thumb)
    }
}

/// The track a shade lighter, for the pointer's answer.
fn lift(c: qs_gpu::color::Srgba) -> qs_gpu::color::Srgba {
    crate::oklab::mix(qs_gpu::color::Srgba::new(1.0, 1.0, 1.0, 1.0), c, 0.08)
}

#[cfg(test)]
#[path = "switch.tests.rs"]
mod tests;
