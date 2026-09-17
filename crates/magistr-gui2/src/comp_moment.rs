use magistr_application::sessions::PermissionKind;
use magistr_cockpit::cockpit::{Model, Permission};
use magistr_cockpit::theme::Severity;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::card::Card;
use crate::component::Component;
use crate::diff_block::DiffBlock;
use crate::diff_row::DiffRow;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::receipt_row::ReceiptRow;
use crate::severity_cap::SeverityCap;
use crate::status_row::StatusRow;
use crate::voice::Voice;

/// The Moments, composed from the model: one decision, everything needed to make it,
/// nothing else.
///
/// An **ask** ([`Model::moment`]) is the centred plan card: the peach cap, the ask's
/// own title, the numbered steps, the checkpoint the spine minted, and the answers as
/// the card's verbs. A **review** ([`Model::review`]) replaces the session: the scope
/// line, one status row per journal file, the selected file's diff in the diff
/// vocabulary, the agent's one-line why, and the same answer verbs. `None` while
/// nothing asks — the predicate is the model's, never re-derived here.
pub struct MomentComposition;

impl MomentComposition {
    /// The id space the answer verbs index into, by
    /// [`PermissionKind`]'s own order.
    pub const ANSWERS: u64 = 240;

    /// The moment's card measure.
    const WIDTH: f32 = 470.0;

    /// The Moment for `model`, in `medium`'s lexicon — the review when
    /// one is up (it replaces the session), else the plan ask, else
    /// `None`.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Option<Element> {
        if let Some(permission) = model.review() {
            return Some(review(model, permission, medium));
        }
        if let Some(ask) = model.moment() {
            return Some(plan_card(ask, medium));
        }
        model.ask().map(|ask| ask_card(ask, medium))
    }
}

/// The answer verbs, in the model's own order — allow leads, and each
/// id is the kind's slot in [`MomentComposition::ANSWERS`].
fn answers(permission: &Permission, medium: Medium) -> Element {
    let mut primary = None;
    let mut rest = Vec::new();
    for (kind, label) in permission.buttons() {
        let slot = PermissionKind::ALL
            .iter()
            .position(|k| *k == kind)
            .unwrap_or(0) as u64;
        let action = Action::new(label, ElementId(MomentComposition::ANSWERS + slot));
        match kind {
            PermissionKind::AllowOnce if primary.is_none() => primary = Some(action),
            PermissionKind::RejectOnce | PermissionKind::RejectAlways => {
                rest.push(action.negative());
            }
            _ => rest.push(action),
        }
    }
    ActionRow::new(primary, rest).gap(8.0).el(medium)
}

/// The centred plan card of the ask Moment.
fn plan_card(ask: &Permission, medium: Medium) -> Element {
    let lex = Lexicon::of(medium);
    let mut card = Card::overlay(lex.base, 16.0, 14.0, &lex)
        .width(MomentComposition::WIDTH)
        .gap(8.0)
        .align_start()
        .child(SeverityCap::el(
            Severity::Attention,
            None,
            "plan \u{b7} two-phase",
            &lex,
        ))
        .child(
            Element::text(Voice::SLOT, lex.tx, ask.title())
                .claim(Component::Title)
                .nowrap(),
        );
    for (number, step) in ask.steps().enumerate() {
        card = card.child(
            Element::text(Voice::SLOT, lex.sb1, format!("{}. {step}", number + 1))
                .claim(Component::ListRow)
                .nowrap(),
        );
    }
    receipts(card, ask, &lex).child(answers(ask, medium))
}

/// The centred card of a plain ask: the tool the agent is blocked on under a *needs
/// you* cap — the strip's own words, the ones `Cmd::Announce` already says — then what
/// the plan card also carries, and the same answer verbs, so a click reaches `update`
/// as the strip's own Msg. The one ask a live session ever raises: ACP's request
/// carries a tool title and no plan.
fn ask_card(ask: &Permission, medium: Medium) -> Element {
    let lex = Lexicon::of(medium);
    let card = Card::overlay(lex.base, 16.0, 14.0, &lex)
        .width(MomentComposition::WIDTH)
        .gap(8.0)
        .align_start()
        .child(SeverityCap::el(
            Severity::Attention,
            None,
            "needs you",
            &lex,
        ))
        .child(
            Element::text(Voice::SLOT, lex.tx, ask.title())
                .claim(Component::Title)
                .nowrap(),
        );
    receipts(card, ask, &lex).child(answers(ask, medium))
}

/// What both ask cards carry under their title: the checkpoint the spine minted when
/// the ask was High risk, and the scope a `⇧A` would cover.
fn receipts(mut card: Element, ask: &Permission, lex: &Lexicon) -> Element {
    if let Some(checkpoint) = ask.checkpoint() {
        card = card.child(
            Element::surface()
                .width(MomentComposition::WIDTH - 2.0 * 16.0)
                .child(ReceiptRow::value(
                    (crate::icon::Icon::Commit, "saved"),
                    checkpoint,
                    None,
                    false,
                    lex.s1,
                    lex,
                )),
        );
    }
    if let Some(scope) = ask.scope() {
        card = card.child(
            Element::text(Voice::CAP, lex.ov0, format!("always allow ({scope})"))
                .claim(Component::Cap)
                .nowrap(),
        );
    }
    card
}

/// The review Moment — the change is the thing to look at.
fn review(model: &Model, permission: &Permission, medium: Medium) -> Element {
    let lex = Lexicon::of(medium);
    let mut column = Element::surface()
        .fill(lex.base)
        .pad(18.0)
        .gap(10.0)
        .align_start()
        .child(SeverityCap::el(Severity::Attention, None, "Review", &lex))
        .child(
            Element::text(Voice::CAP, lex.ov0, model.review_scope())
                .claim(Component::Cap)
                .nowrap(),
        );
    let mut files = Element::surface().width(MomentComposition::WIDTH);
    for entry in model.review_files() {
        let (verdict, ink) = if entry.parse_ok {
            ("parses", None)
        } else {
            ("refused", Some(lex.red))
        };
        files = files.child(StatusRow::el(
            if entry.parse_ok { lex.green } else { lex.red },
            &entry.path,
            (verdict, ink),
            &format!("+{} \u{2212}{}", entry.added, entry.removed),
            true,
            lex.base,
            &lex,
        ));
    }
    column = column.child(files);
    if let Some(entry) = model.review_file() {
        let rows = entry
            .diff
            .lines()
            .map(|line| match line.as_bytes().first() {
                Some(b'+') => DiffRow::add(line, lex.s0, &lex),
                Some(b'-') => DiffRow::del(line, lex.s0, &lex),
                _ => DiffRow::ctx(line, &lex),
            })
            .collect();
        column = column.child(Element::surface().width(MomentComposition::WIDTH).child(
            DiffBlock::el(&format!("diff \u{b7} {}", entry.path), rows, lex.base, &lex),
        ));
    }
    if let Some(note) = model.review_note() {
        column = column.child(
            Element::text(Voice::CAP, lex.sb0, note)
                .claim(Component::Cap)
                .nowrap(),
        );
    }
    column.child(answers(permission, medium))
}

#[cfg(test)]
#[path = "comp_moment.tests.rs"]
mod tests;
