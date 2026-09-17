use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::component::Component;
use crate::element::Element;
use crate::elevation::Elevation;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::oklab;
use crate::voice::Voice;

/// A screen's actions: at most one **primary** — the one lit exit the design allows —
/// beside its secondaries. The law is the type: the primary is a single `Option` slot,
/// so a composition carrying two cannot be constructed, and `one_lit_exit` stays
/// assertable over any built tree by counting green gradients.
///
/// Faces are `.btn7`'s own, derived live from the volume's palette through the CSS
/// `color-mix(in oklab, …)` recipes — night and day each mix their own (day is not
/// inverted night).
pub struct ActionRow {
    primary: Option<Action>,
    rest: Vec<Action>,
    gap: f32,
}

/// White and black, the two mix partners `.btn7`'s recipes name.
const WHITE: Srgba = Srgba::new(1.0, 1.0, 1.0, 1.0);
const BLACK: Srgba = Srgba::new(0.0, 0.0, 0.0, 1.0);

impl ActionRow {
    /// The border-box paddings (`padding:6px 14px; border:1px`) and the button's radius
    /// — raised rect at 9px.
    const PAD_X: f32 = 15.0;
    const PAD_Y: f32 = 7.0;
    const RADIUS: f32 = 9.0;
    /// The gap between the verb and its key cap (`.btn7 { gap:8px }`),
    /// and between adjacent buttons.
    const GAP: f32 = 8.0;
    const ROW_GAP: f32 = 10.0;

    /// A row holding its one possible primary and everything else.
    #[must_use]
    pub fn new(primary: Option<Action>, rest: Vec<Action>) -> Self {
        Self {
            primary,
            rest,
            gap: Self::ROW_GAP,
        }
    }

    /// The space between adjacent buttons, where a card's row is
    /// tighter than the sheet's default (the volume writes 6, 8 and
    /// 10 in different compositions).
    #[must_use]
    pub fn gap(mut self, px: f32) -> Self {
        self.gap = px;
        self
    }

    /// The row, primary first — the lit exit reads before its
    /// alternatives.
    #[must_use]
    pub fn el(&self, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let mut row = Element::row().gap(self.gap).center();
        if let Some(primary) = &self.primary {
            row = row.child(Self::button(primary, true, medium, &lex));
        }
        for action in &self.rest {
            row = row.child(Self::button(action, false, medium, &lex));
        }
        row
    }

    /// One raised action. The primary wears the green gradient and its
    /// own ink, an accent the mauve raise; everything else the neutral
    /// one.
    fn button(action: &Action, primary: bool, medium: Medium, lex: &Lexicon) -> Element {
        let face = face(primary, action.accent, medium, lex);
        let mut el = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .radius(Self::RADIUS);
        if action.disabled {
            el = el.disabled().fill(lex.s0).stroke(lex.hairc, 1.0).child(
                Element::text(Voice::BUTTON, lex.ov0, action.verb.as_str())
                    .claim(Component::Button)
                    .nowrap(),
            );
            return el;
        }
        el = el
            .gradient(face.top, face.bottom)
            .stroke(face.border, 1.0)
            .elevation(Elevation::raised())
            .hover_fill(oklab::mix(WHITE, face.top, 0.08))
            .press_fill(oklab::mix(BLACK, face.bottom, 0.08))
            .id(action.id)
            .focusable()
            .focus_ring(lex.mauve)
            .child(
                Element::text(Voice::BUTTON, face.ink, action.verb.as_str())
                    .claim(Component::Button)
                    .nowrap(),
            );
        if let Some(key) = &action.cap {
            el = el.child(cap(key, primary || action.accent, medium, face.ink));
        }
        el
    }
}

/// A button face: the vertical ramp, its border and its ink.
struct Face {
    top: Srgba,
    bottom: Srgba,
    border: Srgba,
    ink: Srgba,
}

/// `.btn7` / `.btn7.pri` / `.btn7.acc`'s face for `medium`, mixed the
/// way the CSS mixes it. The literals the volume writes outside its
/// palette — the primary's night ink `#0d1c0e`, the accent's night ink
/// `#180f2b` and the day secondary's paper ramp — are carried here
/// with their CSS as the comment.
fn face(primary: bool, accent: bool, medium: Medium, lex: &Lexicon) -> Face {
    if accent && !primary {
        return match medium {
            // .btn7.acc: color:#180f2b;
            // linear-gradient(180deg, mix(#fff 16%, mauve), mix(#000 8%, mauve));
            // border mix(#fff 26%, mauve)
            Medium::Night => Face {
                top: oklab::mix(WHITE, lex.mauve, 0.16),
                bottom: oklab::mix(BLACK, lex.mauve, 0.08),
                border: oklab::mix(WHITE, lex.mauve, 0.26),
                ink: Srgba::new(
                    0x18 as f32 / 255.0,
                    0x0f as f32 / 255.0,
                    0x2b as f32 / 255.0,
                    1.0,
                ),
            },
            // day .btn7.acc: color:#fff; mix(#fff 12%, mauve) → mix(#000 10%, mauve);
            // border mix(#000 14%, mauve)
            Medium::Day => Face {
                top: oklab::mix(WHITE, lex.mauve, 0.12),
                bottom: oklab::mix(BLACK, lex.mauve, 0.10),
                border: oklab::mix(BLACK, lex.mauve, 0.14),
                ink: WHITE,
            },
        };
    }
    match (primary, medium) {
        // .btn7.pri: linear-gradient(180deg, mix(#fff 18%, green), mix(#000 8%, green))
        (true, Medium::Night) => Face {
            top: oklab::mix(WHITE, lex.green, 0.18),
            bottom: oklab::mix(BLACK, lex.green, 0.08),
            border: oklab::mix(WHITE, lex.green, 0.28),
            // .btn7.pri { color:#0d1c0e }
            ink: Srgba::new(
                0x0d as f32 / 255.0,
                0x1c as f32 / 255.0,
                0x0e as f32 / 255.0,
                1.0,
            ),
        },
        // day .btn7.pri: color:#fff; mix(#fff 14%, green) → mix(#000 10%, green)
        (true, Medium::Day) => Face {
            top: oklab::mix(WHITE, lex.green, 0.14),
            bottom: oklab::mix(BLACK, lex.green, 0.10),
            border: oklab::mix(BLACK, lex.green, 0.16),
            ink: WHITE,
        },
        // .btn7: linear-gradient(180deg, mix(#fff 9%, s0), s0); border mix(#fff 13%, s0)
        (false, Medium::Night) => Face {
            top: oklab::mix(WHITE, lex.s0, 0.09),
            bottom: lex.s0,
            border: oklab::mix(WHITE, lex.s0, 0.13),
            ink: lex.tx,
        },
        // day .btn7: linear-gradient(180deg, #ffffff, #f1f0f7); border #c7c5d6
        (false, Medium::Day) => Face {
            top: WHITE,
            bottom: Srgba::new(
                0xf1 as f32 / 255.0,
                0xf0 as f32 / 255.0,
                0xf7 as f32 / 255.0,
                1.0,
            ),
            border: Srgba::new(
                0xc7 as f32 / 255.0,
                0xc5 as f32 / 255.0,
                0xd6 as f32 / 255.0,
                1.0,
            ),
            ink: lex.tx,
        },
    }
}

/// The key cap inside a button: `.kbd7` — a mono data pill in the
/// button's own ink at 85% (`opacity:.85`), grounded per face.
fn cap(key: &str, primary: bool, medium: Medium, ink: Srgba) -> Element {
    // .btn7.pri .kbd7 { background:rgba(0,0,0,.18); border-color:rgba(0,0,0,.22) }
    // .kbd7 (night)   { background:rgba(0,0,0,.28); border:1px solid rgba(255,255,255,.14) }
    // .kbd7 (day)     { background:rgba(64,60,90,.08); border-color:rgba(64,60,90,.18) }
    let slate = |a: f32| Srgba::new(64.0 / 255.0, 60.0 / 255.0, 90.0 / 255.0, a);
    let (bg, border) = if primary {
        (
            Srgba::new(0.0, 0.0, 0.0, 0.18),
            Srgba::new(0.0, 0.0, 0.0, 0.22),
        )
    } else if medium == Medium::Night {
        (
            Srgba::new(0.0, 0.0, 0.0, 0.28),
            Srgba::new(1.0, 1.0, 1.0, 0.14),
        )
    } else {
        (slate(0.08), slate(0.18))
    };
    Element::row()
        .pad_x(7.0)
        .pad_y(2.0)
        .center()
        .radius(5.0)
        .fill(bg)
        .stroke(border, 1.0)
        .child(
            Element::text(
                Voice::KBD,
                Srgba::new(ink.r, ink.g, ink.b, ink.a * 0.85),
                key,
            )
            .claim(Component::Button)
            .nowrap(),
        )
}

#[cfg(test)]
#[path = "action_row.tests.rs"]
mod tests;
