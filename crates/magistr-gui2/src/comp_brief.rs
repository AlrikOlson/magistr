use magistr_cockpit::cockpit::{BriefBody, Model};

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::severity_cap::SeverityCap;
use crate::thin_bar::ThinBar;
use crate::voice::Voice;

/// The Brief, composed from the model: what happened while you were away, as
/// [`Brief::of`](magistr_cockpit::cockpit::Brief::of) already sorted it — a glass
/// overlay of severity cards dealt worst-first, each band's tinted ground and cap, one
/// verb per card, and the dismiss pair. `None` exactly when the model says there is no
/// Brief ([`Model::brief`]); the rise timing is [`Model::brief_rise`]'s — a card the
/// deal has not reached holds its ghost silhouette, and nothing here counts ticks of
/// its own.
pub struct BriefComposition;

impl BriefComposition {
    /// The dismiss action.
    pub const DISMISS: ElementId = ElementId(220);
    /// The id space the cards' verbs index into, by card.
    pub const VERBS: u64 = 224;

    /// The overlay's column — the volume's Brief measure.
    const WIDTH: f32 = 470.0;
    const PAD_X: f32 = 16.0;
    const PAD_Y: f32 = 14.0;

    /// The Brief for `model`, in `medium`'s lexicon — `None` on a quiet
    /// project, a terminal, or while something is asking.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Option<Element> {
        let brief = model.brief()?;
        let lex = Lexicon::of(medium);
        let mut column = Card::overlay(lex.base, Self::PAD_X, Self::PAD_Y, &lex)
            .width(Self::WIDTH)
            .gap(10.0)
            .align_start();
        column = column.child(
            Element::row()
                .center()
                .gap(10.0)
                .child(
                    Element::text(Voice::CAP, lex.ov0, "Summary")
                        .claim(Component::Cap)
                        .nowrap(),
                )
                .child(
                    Element::text(Voice::CAP, lex.sb0, format!("away {}", brief.away()))
                        .claim(Component::Cap)
                        .nowrap(),
                )
                .child(Element::row().grow(1.0))
                .child(
                    ActionRow::new(None, vec![Action::new("Dismiss", Self::DISMISS).cap("esc")])
                        .el(medium),
                ),
        );
        for (index, card) in brief.cards().iter().enumerate() {
            let undealt = model.brief_rise(index).is_some_and(|t| t <= f32::EPSILON);
            column = column.child(if undealt {
                ghost(&lex)
            } else {
                dealt(card, index, &lex, medium)
            });
        }
        Some(column)
    }
}

/// One landed card: its band's tinted ground and cap, the body's
/// sentence with its one verb — or its measure with the readout and
/// the thin meter.
fn dealt(
    card: &magistr_cockpit::cockpit::BriefCard,
    index: usize,
    lex: &Lexicon,
    medium: Medium,
) -> Element {
    let band = card.severity();
    let mut float = Card::severity(band, 14.0, 10.0, lex)
        .gap(6.0)
        .align_start()
        .child(SeverityCap::el(band, None, card.cap(), lex));
    match card.body() {
        BriefBody::Act { text, verb, .. } => {
            float = float
                .child(
                    Element::text(Voice::SLOT, lex.tx, text)
                        .claim(Component::Slot)
                        .nowrap(),
                )
                .child(
                    ActionRow::new(
                        Some(Action::new(
                            *verb,
                            ElementId(BriefComposition::VERBS + index as u64),
                        )),
                        Vec::new(),
                    )
                    .el(medium),
                );
        }
        BriefBody::Measure { readout, fill, .. } => {
            float = float
                .child(
                    Element::text(Voice::VALUE, lex.sb1, readout)
                        .claim(Component::Meter)
                        .nowrap(),
                )
                .child(ThinBar::el(
                    f32::from(*fill) / 1000.0,
                    BriefComposition::WIDTH - 2.0 * 14.0,
                    lex,
                ));
        }
    }
    float
}

/// The silhouette a card holds until the deal reaches it.
fn ghost(lex: &Lexicon) -> Element {
    Element::surface()
        .height(46.0)
        .radius(13.0)
        .stroke(lex.grounded(lex.hairc), 1.0)
        .dashed()
        .center()
        .justify_center()
        .child(
            Element::text(Voice::CAP, lex.ov0, "dealing\u{2026}")
                .claim(Component::Cap)
                .nowrap(),
        )
}

#[cfg(test)]
#[path = "comp_brief.tests.rs"]
mod tests;
