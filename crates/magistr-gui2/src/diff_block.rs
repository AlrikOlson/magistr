use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A diff in its frame — the volume's bordered block: a hairline ring
/// at the panel radius, the file named in a mono header over its own
/// hairline, the [`DiffRow`](crate::DiffRow)s under it. The frame is a
/// *block boundary*, not elevation — a diff sits in the page, it does
/// not float over it — which is why this is the one surface here that
/// legitimately wears a 1px ring.
pub struct DiffBlock;

impl DiffBlock {
    /// The block's `border-radius:11px` — the panel radius, one step
    /// under the card's.
    const RADIUS: f32 = 11.0;
    /// The header's `padding:6px 12px`.
    const HEADER_PAD_X: f32 = 12.0;
    const HEADER_PAD_Y: f32 = 6.0;

    /// The block: `title` in the header, `rows` under it, framed on
    /// `ground`.
    #[must_use]
    pub fn el(title: &str, rows: Vec<Element>, ground: Srgba, lex: &Lexicon) -> Element {
        let hair = Lexicon::composite(ground, lex.hairc);
        let mut block = Element::surface()
            .radius(Self::RADIUS)
            .stroke(hair, 1.0)
            .child(
                Element::surface()
                    .child(
                        Element::row()
                            .pad_x(Self::HEADER_PAD_X)
                            .pad_y(Self::HEADER_PAD_Y)
                            .child(
                                Element::text(Voice::CHIP, lex.sb0, title)
                                    .claim(Component::Diff)
                                    .nowrap(),
                            ),
                    )
                    .child(Element::surface().height(1.0).fill(hair)),
            );
        for row in rows {
            block = block.child(row);
        }
        block
    }
}

#[cfg(test)]
#[path = "diff_block.tests.rs"]
mod tests;
