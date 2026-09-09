use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// Selection, for enumerable text: segmented pills, the chosen one filled —
/// the volume's `The sky · The chart · Follow OS` stadium. Flat, never raised: a choice
/// is not an action.
pub struct Segments;

impl Segments {
    /// Each segment's padding (`padding:4px 12px`); the shell's border
    /// puts one more pixel around the whole group.
    const PAD_X: f32 = 12.0;
    const PAD_Y: f32 = 4.0;
    /// How far a filled end segment's squaring patch reaches back from
    /// the seam — the stadium curve it must cover.
    const SEAM: f32 = 13.0;

    /// The group over `choices` with `chosen` filled. Segment `i`
    /// answers on `ElementId(base.0 + i)`.
    #[must_use]
    pub fn el(choices: &[&str], chosen: usize, base: ElementId, lex: &Lexicon) -> Element {
        let mut shell = Element::row()
            .radius(999.0)
            .stroke(lex.grounded(lex.hairc), 1.0)
            // The shell's border-box: children sit inside the 1px ring.
            .pad(1.0);
        for (index, choice) in choices.iter().enumerate() {
            let picked = index == chosen;
            let voice = Voice {
                weight: if picked { 600 } else { Voice::CHIP.weight },
                ..Voice::CHIP
            };
            let ink = if picked { lex.base } else { lex.sb0 };
            let mut segment = Element::row()
                .pad_x(Self::PAD_X)
                .pad_y(Self::PAD_Y)
                .center()
                .id(ElementId(base.0 + index as u64))
                .focusable()
                .focus_ring(lex.mauve);
            if picked {
                segment = segment.fill(lex.mauve);
                // The shell clips its children in CSS (`overflow:
                // hidden` on a stadium); this painter does not clip, so
                // a filled END segment follows the stadium's curve
                // itself — rounded on the outside, squared back on the
                // seam side by a patch as wide as the curve.
                let at_end = choices.len() > 1 && (index == 0 || index + 1 == choices.len());
                if at_end {
                    segment = segment.radius(999.0);
                    let patch = Element::surface()
                        .width(Self::SEAM)
                        .fill(lex.mauve)
                        .pin_top(0.0)
                        .pin_bottom(0.0);
                    segment = segment.child(if index == 0 {
                        patch.pin_right(0.0)
                    } else {
                        patch.pin_left(0.0)
                    });
                }
            } else {
                segment = segment.hover_fill(lex.mantle).press_fill(lex.deep);
            }
            shell = shell.child(
                segment.child(
                    Element::text(voice, ink, *choice)
                        .claim(Component::Chip)
                        .nowrap(),
                ),
            );
        }
        shell
    }
}

#[cfg(test)]
#[path = "segments.tests.rs"]
mod tests;
