use magistr_cockpit::cockpit::{Model, Palette, PaletteRow};
use qs_gpu::color::Srgba;

use crate::card::Card;
use crate::cmdk_row::CmdkRow;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::medium::Medium;
use crate::sigil::Sigil;
use crate::voice::Voice;

/// The ⌘K palette: the volume's popover specimen (II.d, view 01) driven by the model —
/// the glass card floating near the top of the window, the chord header over its
/// hairline, one washed row for the chosen result and the pending act under it with its
/// `⌘↵`.
///
/// A true overlay with **no scrim**: the room behind stays fully lit, because the
/// palette is a jump, not a decision — Moment dims the world, ⌘K floats over it.
pub struct PaletteComposition;

impl PaletteComposition {
    /// The result rows' element ids: `ROWS + index` for the row at
    /// `index` in [`Palette::rows`], `ROWS_END` exclusive.
    pub const ROWS: u64 = 400;
    pub const QUERY: ElementId = ElementId(433);
    /// One indexed space's width — far more rows than the palette cuts
    /// its list to.
    pub const ROWS_END: u64 = 432;

    /// The card's measure and seat in the capture: view 01's popover
    /// is 400px wide with its top edge 107px under the window's.
    const WIDTH: f32 = 400.0;
    const TOP: f32 = 107.0;
    /// The card's own padding (`.ov7`'s inset around the rows).
    const PAD_X: f32 = 6.0;
    const PAD_Y: f32 = 4.0;
    /// The header row's padding and gap.
    const HEAD_PAD_X: f32 = 10.0;
    const HEAD_PAD_Y: f32 = 7.0;
    const HEAD_GAP: f32 = 10.0;
    /// The caret: the accent at the query's writing edge.
    const CARET: (f32, f32) = (1.5, 14.0);
    /// The act sub-row's padding and gaps (the volume's pending row —
    /// its 36px box, a shade deeper than the result rows').
    const ACT_PAD_X: f32 = 12.0;
    const ACT_PAD_Y: f32 = 8.5;
    const ACT_GAP: f32 = 6.0;
    /// The sigil's side and the alert glyph's.
    const SIGIL_PX: u32 = 14;
    const ALERT_PX: u32 = 13;

    /// The palette float for `model`, pinned over the whole window —
    /// `None` while the palette is down.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Option<Element> {
        let palette = model.palette()?;
        let lex = Lexicon::of(medium);
        let glass = Lexicon::composite(
            lex.base,
            Srgba::new(lex.mantle.r, lex.mantle.g, lex.mantle.b, 0.84),
        );
        let mut card = Card::popover(lex.base, Self::PAD_X, Self::PAD_Y, &lex)
            .width(Self::WIDTH)
            .child(header(palette, glass, &lex));
        for (index, row) in Palette::rows(model).into_iter().enumerate() {
            let chosen = index == palette.chosen();
            let act = chosen.then(|| row.act.clone()).flatten();
            card = card.child(CmdkRow::el(
                content(&row, chosen, &lex),
                chosen,
                glass,
                ElementId(Self::ROWS + index as u64),
                &lex,
            ));
            if let Some(act) = act {
                card = card.child(pending(&act, &lex));
            }
        }
        Some(
            Element::surface()
                .pin_left(0.0)
                .pin_top(0.0)
                .pin_right(0.0)
                .pin_bottom(0.0)
                .center()
                .child(Element::surface().height(Self::TOP))
                .child(card),
        )
    }
}

/// The chord header: `⌘K`, what has been typed, and the caret — over
/// the card's own hairline.
fn header(palette: &Palette, glass: Srgba, lex: &Lexicon) -> Element {
    let chord = Voice {
        px: 11.0,
        ..Voice::CHIP
    };
    Element::surface()
        .child(
            Element::row()
                .pad_x(PaletteComposition::HEAD_PAD_X)
                .pad_y(PaletteComposition::HEAD_PAD_Y)
                .gap(PaletteComposition::HEAD_GAP)
                .center()
                .child(
                    Element::text(chord, lex.mauve, "\u{2318}K")
                        .claim(Component::KeyHint)
                        .nowrap(),
                )
                .child(
                    Element::row()
                        .center()
                        // What is being typed is set in the mono
                        // family — the volume's input register, not
                        // its reading one.
                        .child(
                            Element::text(
                                Voice {
                                    family: "mono",
                                    px: 12.0,
                                    ..Voice::SLOT
                                },
                                lex.tx,
                                palette.query(),
                            )
                            .claim(Component::Field)
                            .min_width(12.0)
                            .id(PaletteComposition::QUERY)
                            .label("Commands")
                            .focusable()
                            .focus_ring(lex.mauve)
                            .nowrap(),
                        )
                        .child(
                            Element::surface()
                                .width(PaletteComposition::CARET.0)
                                .height(PaletteComposition::CARET.1)
                                .fill(lex.mauve),
                        ),
                ),
        )
        .child(
            Element::surface()
                .height(1.0)
                .fill(Lexicon::composite(glass, lex.hairc)),
        )
}

/// One result's content, in the shell's own gap: the lead (verb, and
/// the sigil-marked identity a place carries), the live badge, and the
/// right-edge hint — `↵` on the chosen row, a binding's keys on an
/// idle one.
fn content(row: &PaletteRow, chosen: bool, lex: &Lexicon) -> Vec<Element> {
    let mut lead = Element::row().center();
    match &row.name {
        Some(name) => {
            lead = lead.child(
                Element::text(body(), lex.tx, format!("{}\u{a0}", row.verb))
                    .claim(Component::CommandRow)
                    .nowrap(),
            );
            if let Some(name) = row
                .sigil
                .then(|| {
                    Sigil::VOLUME_PROJECTS
                        .into_iter()
                        .find(|p| *p == name.as_str())
                })
                .flatten()
            {
                // The sigil is a pure function of a 'static name
                // (`Mark`'s contract, the header's own rule) — a place
                // outside the volume's four goes unmarked.
                lead = lead.child(Element::mark(
                    Mark::Sigil(name),
                    PaletteComposition::SIGIL_PX,
                    Lexicon::ink_bytes(lex.tx),
                ));
            }
            lead = lead.child(
                Element::text(
                    Voice {
                        px: 12.0,
                        weight: 700,
                        ..Voice::SLOT
                    },
                    lex.tx,
                    name.as_str(),
                )
                .claim(Component::CommandRow)
                .nowrap(),
            );
        }
        None => {
            lead = lead.child(
                Element::text(body(), lex.tx, row.verb.as_str())
                    .claim(Component::CommandRow)
                    .nowrap(),
            )
        }
    }
    let mut content = vec![lead];
    if let Some(badge) = &row.badge {
        content.push(
            Element::text(Voice::LEDGER_META, lex.peach, badge.as_str())
                .claim(Component::CommandRow)
                .nowrap(),
        );
    }
    content.push(Element::row().grow(1.0));
    if chosen {
        content.push(
            Element::text(Voice::KBD, lex.ov0, "\u{21b5}")
                .claim(Component::KeyHint)
                .nowrap(),
        );
    } else if let Some(keys) = row.keys {
        content.push(
            Element::text(Voice::KBD, lex.ov0, keys)
                .claim(Component::KeyHint)
                .nowrap(),
        );
    }
    content
}

/// The chosen result's pending act, under it at full width: the alert,
/// what ⌘↵ would do, and the chord itself at the right edge.
fn pending(act: &str, lex: &Lexicon) -> Element {
    Element::row()
        .pad_x(PaletteComposition::ACT_PAD_X)
        .pad_y(PaletteComposition::ACT_PAD_Y)
        .center()
        .child(
            Element::row()
                .gap(PaletteComposition::ACT_GAP)
                .center()
                .child(Element::mark(
                    Mark::Icon(Icon::Alert),
                    PaletteComposition::ALERT_PX,
                    Lexicon::ink_bytes(lex.sb0),
                ))
                .child(
                    Element::text(body(), lex.sb0, act)
                        .claim(Component::CommandRow)
                        .nowrap(),
                ),
        )
        .child(Element::row().grow(1.0))
        .child(
            Element::text(Voice::KBD, lex.ov0, "\u{2318}\u{21b5}")
                .claim(Component::KeyHint)
                .nowrap(),
        )
}

/// The palette's reading voice — the volume's 12px slot.
fn body() -> Voice {
    Voice {
        px: 12.0,
        ..Voice::SLOT
    }
}

#[cfg(test)]
#[path = "comp_palette.tests.rs"]
mod tests;
