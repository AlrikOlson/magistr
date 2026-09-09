use magistr_cockpit::cockpit::{Fault, Model, StateWord};

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::pill::DataPill;
use crate::severity::SeverityInk;
use crate::severity_cap::SeverityCap;
use crate::status_row::StatusRow;
use crate::voice::Voice;

/// The bad day, composed from the model: no error screens — the faults
/// [`BadDay::of`](magistr_cockpit::cockpit::BadDay::of) already dealt worst first, each
/// a severity card on its tinted ground with its coloured cap, the one verb that fixes
/// it, and its blast radius in the cap; the header's `N blocking` chip; and at the foot
/// the murmur row of every star a fault rings, in the fault's own words. `None` exactly
/// when the model says nothing is wrong ([`Model::bad_day`]).
///
/// It floats over the room with no scrim — the room is the dimmed constellation, still
/// — and it is never dismissed: a card leaves when the fact behind it does.
pub struct BadDayComposition;

impl BadDayComposition {
    /// The id space the cards' verbs index into, across every card in
    /// deal order — [`BadDay::verb`](magistr_cockpit::cockpit::BadDay::verb)
    /// answers the widget behind each.
    pub const VERBS: u64 = 0x2100;
    /// One past the last verb id.
    pub const VERBS_END: u64 = 0x2140;

    /// The column's measure — the volume's `width:540px`.
    const WIDTH: f32 = 540.0;
    /// The column's centre sits at 47% of the field's height
    /// (`top:47%; transform:translateY(-50%)`): pinned top 0 and bottom
    /// 6% of the field, flex centring lands it there exactly.
    const CENTRE: f32 = 0.47;
    /// The air between cards (`margin-bottom:10px`).
    const CARD_GAP: f32 = 10.0;
    /// The titled card's paddings (`padding:14px 17px`).
    const TITLED_PAD_X: f32 = 17.0;
    const TITLED_PAD_Y: f32 = 14.0;
    /// A row card's paddings (`padding:12px 15px`).
    const ROW_PAD_X: f32 = 15.0;
    const ROW_PAD_Y: f32 = 12.0;
    /// A row card's flex gap.
    const ROW_GAP: f32 = 10.0;
    /// The titled card's own air: under the cap, under the title,
    /// under the note, and between its buttons.
    const CAP_GAP: f32 = 4.0;
    const TITLE_GAP: f32 = 3.0;
    const NOTE_GAP: f32 = 10.0;
    const BUTTON_GAP: f32 = 8.0;
    /// The murmur foot: its measure and its lift off the field's foot
    /// (`width:360px; bottom:22px`).
    const FOOT_WIDTH: f32 = 360.0;
    const FOOT_LIFT: f32 = 22.0;
    /// Murmur rows the foot keeps — the resting sky's own cap.
    const FOOT_ROWS: usize = 2;

    /// The titled card's headline — `font-size:14px; font-weight:600`.
    const TITLE: Voice = Voice {
        px: 14.0,
        weight: 600,
        ..Voice::SLOT
    };
    /// The titled card's mono note — `.mono` at 10.5.
    const NOTE: Voice = Voice {
        px: 10.5,
        ..Voice::LEDGER_META
    };

    /// The header's chip: how many faults stop a session, in the alert
    /// hue with its dot — `● 2 blocking`. `None` on a good day.
    #[must_use]
    pub fn chips(model: &Model, medium: Medium) -> Option<Element> {
        let day = model.bad_day()?;
        let lex = Lexicon::of(medium);
        Some(DataPill::alarm(
            &format!("{} blocking", day.blocking()),
            lex.red,
            &lex,
        ))
    }

    /// The bad day over a field `field_height` tall, in `medium`'s
    /// lexicon — the cards' column and the murmur foot, both out of
    /// flow over the whole field. `None` on a good day.
    #[must_use]
    pub fn el(model: &Model, medium: Medium, field_height: f32) -> Option<Element> {
        let day = model.bad_day()?;
        let lex = Lexicon::of(medium);
        let mut column = Element::surface()
            .width(Self::WIDTH)
            .gap(Self::CARD_GAP)
            .align_start();
        let mut next = Self::VERBS;
        for fault in day.faults() {
            let ids: Vec<ElementId> = fault
                .verbs()
                .iter()
                .map(|_| {
                    let id = ElementId(next);
                    next += 1;
                    id
                })
                .collect();
            column = column.child(card(fault, &ids, &lex, medium).stretch());
        }
        // Centred between the field's top and 6% short of its foot, so
        // the centre lands at 47% — no translate needed.
        let seat = Element::surface()
            .pin_left(0.0)
            .pin_top(0.0)
            .pin_right(0.0)
            .pin_bottom(field_height * (1.0 - 2.0 * Self::CENTRE))
            .center()
            .justify_center()
            .child(column);
        let foot = Element::row()
            .pin_left(0.0)
            .pin_right(0.0)
            .pin_bottom(Self::FOOT_LIFT)
            .justify_center()
            .child(murmur(model, &lex));
        Some(
            Element::surface()
                .pin_left(0.0)
                .pin_top(0.0)
                .pin_right(0.0)
                .pin_bottom(0.0)
                .child(seat)
                .child(foot),
        )
    }
}

/// One card: the titled shape for a fault with a headline (the cap,
/// the title, the note, the verbs stacked) and the row shape for the
/// rest (the cap, the sentence, the verbs on one line).
fn card(fault: &Fault, ids: &[ElementId], lex: &Lexicon, medium: Medium) -> Element {
    let band = fault.severity();
    let actions: Vec<Action> = fault
        .verbs()
        .iter()
        .zip(ids)
        .map(|(verb, id)| {
            let action = Action::new(verb.verb(), *id);
            if verb.is_lit() {
                action.accent()
            } else {
                action
            }
        })
        .collect();
    match fault.title() {
        Some(title) => Card::severity(
            band,
            BadDayComposition::TITLED_PAD_X,
            BadDayComposition::TITLED_PAD_Y,
            lex,
        )
        .align_start()
        .child(SeverityCap::el(band, None, &fault.cap(), lex))
        .child(air(BadDayComposition::CAP_GAP))
        .child(
            Element::text(BadDayComposition::TITLE, lex.tx, title)
                .claim(Component::Title)
                .nowrap(),
        )
        .child(air(BadDayComposition::TITLE_GAP))
        .child(
            Element::text(BadDayComposition::NOTE, lex.ov0, fault.text())
                .claim(Component::Prose)
                .nowrap(),
        )
        .child(air(BadDayComposition::NOTE_GAP))
        .child(
            ActionRow::new(None, actions)
                .gap(BadDayComposition::BUTTON_GAP)
                .el(medium),
        ),
        None => Card::severity(
            band,
            BadDayComposition::ROW_PAD_X,
            BadDayComposition::ROW_PAD_Y,
            lex,
        )
        .child(
            Element::row()
                .gap(BadDayComposition::ROW_GAP)
                .center()
                // The cap keeps its line: a flex item at its own
                // measure, the sentence beside it is what wraps.
                .child(
                    Element::text(Voice::CAP, SeverityInk::of(band, lex).cap, fault.cap())
                        .claim(Component::Cap)
                        .nowrap(),
                )
                .child(Element::surface().grow(1.0).min_width(0.0).child(
                    Element::text(Voice::STEER, lex.tx, fault.text()).claim(Component::Prose),
                ))
                .child(
                    ActionRow::new(None, actions)
                        .gap(BadDayComposition::ROW_GAP)
                        .el(medium),
                ),
        ),
    }
}

/// The foot's murmur: the stars a session is on or a fault rings, in
/// the state the constellation draws them
/// ([`Model::star_state`]) — so a ringed star reads Blocked here too —
/// with the fault's note as the meta where there is one.
fn murmur(model: &Model, lex: &Lexicon) -> Element {
    let mut rows = Element::surface().width(BadDayComposition::FOOT_WIDTH);
    let chunks: Vec<_> = model
        .roadmap()
        .rows()
        .into_iter()
        .filter(|chunk| {
            let state = model.star_state(chunk);
            state.breathes() || state.alerts()
        })
        .take(BadDayComposition::FOOT_ROWS)
        .collect();
    for chunk in chunks {
        let state = model.star_state(chunk);
        let (dot, state_ink) = if state.alerts() {
            (lex.red, Some(lex.red))
        } else {
            (lex.mauve, None)
        };
        let note = model.star_note(&chunk.id).unwrap_or_default();
        rows = rows.child(
            StatusRow::el(
                dot,
                &chunk.id,
                (StateWord::of_star(state).word(), state_ink),
                &note,
                false,
                lex.base,
                lex,
            )
            .stretch(),
        );
    }
    rows
}

/// A rigid run of vertical air.
fn air(px: f32) -> Element {
    Element::surface().height(px)
}

#[cfg(test)]
#[path = "comp_bad_day.tests.rs"]
mod tests;
