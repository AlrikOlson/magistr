use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;

/// One ⌘K result — the volume's palette row: the chosen row on the
/// accent glow's 13% wash at the row-plus radius, an idle one on
/// nothing at all. The shell is the component; the content — a verb,
/// a sigil, live badges — is the composition's, handed in as children,
/// because a palette row carries whatever the place it names carries.
pub struct CmdkRow;

impl CmdkRow {
    /// `padding:7px 12px; border-radius:9px`.
    const PAD_X: f32 = 12.0;
    const PAD_Y: f32 = 7.0;
    const RADIUS: f32 = 9.0;
    /// The chosen wash — `color-mix(in oklab, var(--glow-m) 13%, transparent)`.
    const WASH: f32 = 0.13;
    /// The row's `gap:10px`.
    const GAP: f32 = 10.0;

    /// The row: `content` in the shell, washed when `chosen`, on
    /// `ground`, answering on `id`.
    #[must_use]
    pub fn el(
        content: Vec<Element>,
        chosen: bool,
        ground: Srgba,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        let mut row = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .radius(Self::RADIUS)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve);
        if chosen {
            let wash = Srgba::new(lex.glow_m.r, lex.glow_m.g, lex.glow_m.b, Self::WASH);
            row = row.fill(Lexicon::composite(ground, wash));
        }
        for child in content {
            row = row.child(child);
        }
        row
    }
}

#[cfg(test)]
#[path = "cmdk_row.tests.rs"]
mod tests;
