use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// slot class: a dashed outline around a place content *goes* — an empty slot or a drop
/// target, and **only** those. Like the data pill it is structurally inert: nothing
/// here can attach an id, take focus, or answer a pointer — a slot is not a button, and
/// only this module draws dashed.
pub struct Slot;

impl Slot {
    /// The volume's inline recipe: `border:1.5px dashed var(--s1);
    /// border-radius:10px; padding:12px 22px; font-size:12px`.
    const BORDER: f32 = 1.5;
    const RADIUS: f32 = 10.0;
    const PAD_X: f32 = 22.0 + Self::BORDER;
    const PAD_Y: f32 = 12.0 + Self::BORDER;

    /// The slot, saying what belongs in it — `Drop a folder here`.
    #[must_use]
    pub fn el(copy: &str, lex: &Lexicon) -> Element {
        Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .center()
            .radius(Self::RADIUS)
            .stroke(lex.s1, Self::BORDER)
            .dashed()
            .child(
                Element::text(Voice::SLOT, lex.sb0, copy)
                    .claim(Component::Slot)
                    .nowrap(),
            )
    }

    /// The empty-state slot — the overlay's big face (`border:1.5px
    /// dashed var(--s1); border-radius:12px; padding:34px 20px;
    /// text-align:center`): the kind's mark large, what belongs here,
    /// and how else to put it there. Still the slot class — only this
    /// module dashes — and still inert.
    #[must_use]
    pub fn empty_state(icon: crate::Icon, copy: &str, hint: &str, lex: &Lexicon) -> Element {
        // The volume's marks-on-empty-states size: `.ic.lg` is 24px.
        const ICON_PX: u32 = 24;
        const RADIUS: f32 = 12.0;
        const PAD_X: f32 = 20.0 + Slot::BORDER;
        const PAD_Y: f32 = 34.0 + Slot::BORDER;
        // The volume's own gaps under the mark and the copy.
        const MARK_GAP: f32 = 8.0;
        const COPY_GAP: f32 = 3.0;
        let copy_voice = Voice {
            px: 13.5,
            ..Voice::SLOT
        };
        Element::surface()
            .pad_x(PAD_X)
            .pad_y(PAD_Y)
            .center()
            .radius(RADIUS)
            .stroke(lex.s1, Slot::BORDER)
            .dashed()
            .child(Element::mark(
                crate::Mark::Icon(icon),
                ICON_PX,
                Lexicon::ink_bytes(lex.sb0),
            ))
            .child(Element::surface().height(MARK_GAP))
            .child(
                Element::text(copy_voice, lex.sb1, copy)
                    .claim(Component::Slot)
                    .nowrap(),
            )
            .child(Element::surface().height(COPY_GAP))
            .child(
                Element::text(Voice::LEDGER_META, lex.ov0, hint)
                    .claim(Component::Slot)
                    .nowrap(),
            )
    }
}

#[cfg(test)]
#[path = "slot.tests.rs"]
mod tests;
