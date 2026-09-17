use magistr_cockpit::cockpit::{Binding, Model};
use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::term_pane::TermPane;
use crate::voice::Voice;

/// The terminal dock on the window ("The dock (16), not a place"): the running card's
/// screen risen from the composer's edge to the dragged height, between the feed and
/// the composer — the feed, the rail and the composer stay, the crumb never moves. A
/// grip row — a hairline over a short handle, the command in the cap voice at the left,
/// the chords at the right in the composer's own hint voice — is what the dock is
/// dragged by; under it the terminal's whole window through [`TermPane::window`], cut
/// to the rows the rise has reached.
///
/// Composed from the model alone: [`Model::docked_term`] says which screen and how many
/// rows at this frame, so a rising, dragged or lowering dock is the same mapping at a
/// different row count.
pub struct TermDockComposition;

impl TermDockComposition {
    /// The grip — the row the dock is dragged by.
    pub const GRIP: ElementId = ElementId(370);
    /// The docked screen — the wheel's target.
    pub const PANE: ElementId = ElementId(371);

    /// The grip row's height.
    pub const GRIP_HEIGHT: f32 = 22.0;
    /// The handle in the grip's middle — a short stadium in the
    /// hairline's ink, the affordance a drag owes you.
    pub const HANDLE: (f32, f32) = (36.0, 4.0);
    /// The air between the dock and the composer under it.
    pub const GAP_BELOW: f32 = 10.0;
    /// The grip's side padding — the pane's own inset.
    const PAD_X: f32 = TermPane::PAD;

    /// The dock for `model`, in `medium`'s lexicon; `None` while it is
    /// down and settled, or while its terminal has written nothing.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Option<Element> {
        let (term, rows) = model.docked_term()?;
        if rows == 0 {
            return None;
        }
        let lex = Lexicon::of(medium);
        let id = model.term_dock().id()?;
        let title = model.turn().terminal_title(id).unwrap_or(id);
        let reduced = !model.theme().motion().is_full();
        // Opaque, in the window's own ground: the dock stands *over*
        // the session, so whatever the feed's last card runs past its
        // box is under the dock on every tier — the GPU's scissor and
        // the CPU still agree pixel for pixel.
        Some(
            Element::surface()
                .stretch()
                .align_start()
                .fill(lex.base)
                .child(Self::grip(title, &lex))
                .child(
                    Element::surface()
                        .stretch()
                        .align_start()
                        .id(Self::PANE)
                        .child(TermPane::window(
                            term,
                            rows,
                            model.ticks(),
                            reduced,
                            lex.base,
                            &lex,
                        )),
                )
                .child(Element::surface().height(Self::GAP_BELOW)),
        )
    }

    /// The chords the grip names: maximize and lower, in the
    /// composer's caps voice.
    #[must_use]
    pub fn hint() -> String {
        format!("{} MAX \u{b7} ESC LOWER", Binding::DOCK_MAX_CHORD.keys)
    }

    /// The grip row: a hairline along its top, the command's cap at
    /// the left, the handle centred, the chords at the right.
    fn grip(title: &str, lex: &Lexicon) -> Element {
        let hair = lex.grounded(lex.hairc);
        let (w, h) = Self::HANDLE;
        Element::surface()
            .stretch()
            .align_start()
            .id(Self::GRIP)
            .hover_fill(Self::raised(lex))
            .child(Element::surface().stretch().height(1.0).fill(hair))
            .child(
                Element::row()
                    .stretch()
                    .height(Self::GRIP_HEIGHT - 1.0)
                    .pad_x(Self::PAD_X)
                    .center()
                    .child(
                        Element::text(Voice::CAP, lex.ov0, title)
                            .claim(Component::Cap)
                            .nowrap()
                            .min_width(0.0),
                    )
                    .child(
                        Element::row().grow(1.0).justify_center().child(
                            Element::surface()
                                .width(w)
                                .height(h)
                                .radius(h / 2.0)
                                .fill(hair),
                        ),
                    )
                    .child(
                        Element::text(Voice::CAP, lex.ov0, Self::hint())
                            .claim(Component::KeyHint)
                            .nowrap()
                            .no_shrink(),
                    ),
            )
    }

    /// The grip under the pointer: the mauve glow at the rail rows'
    /// alpha, the handle's raised ground.
    fn raised(lex: &Lexicon) -> Srgba {
        Lexicon::composite(
            lex.base,
            Srgba::new(lex.glow_m.r, lex.glow_m.g, lex.glow_m.b, 0.09),
        )
    }
}

#[cfg(test)]
#[path = "comp_term_dock.tests.rs"]
mod tests;
