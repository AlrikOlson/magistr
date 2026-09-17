use magistr_cockpit::cockpit::{Model, RoadmapChunk, Shelf, StarState, StateWord};
use magistr_cockpit::theme::Severity;
use qs_gpu::color::Srgba;

use crate::action::Action;
use crate::action_row::ActionRow;
use crate::card::Card;
use crate::comp_shelf::ShelfComposition;
use crate::comp_signals::SignalsComposition;
use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::severity_cap::SeverityCap;
use crate::stat::Stat;
use crate::status_row::StatusRow;
use crate::voice::Voice;

/// The Sky composition derives its awake, rest, and completion states from the shared
/// model.
///
/// Awake views show the counter and controls above the field. At rest, the field
/// remains with one status row per active session. During
/// [`Roadmap::ship_run`](magistr_cockpit::cockpit::Roadmap::ship_run), the completion
/// indicator accompanies the star animation.
///
/// [`Sky`](crate::Sky) draws the constellation into [`SkyComposition::FIELD`].
pub struct SkyComposition;

impl SkyComposition {
    /// The field the constellation is drawn into — a reservation, not
    /// a drawing.
    pub const FIELD: ElementId = ElementId(200);
    /// The selected chunk's \[Start\] verb.
    pub const START: ElementId = ElementId(210);
    /// The selected chunk's \[Complete\] verb.
    pub const COMPLETE: ElementId = ElementId(211);

    /// The detail card's measure, and where it floats off the field's
    /// top-right corner.
    const CARD_WIDTH: f32 = 300.0;
    /// The card's measure once a shipped chunk's shelf is on it: room for a row of
    /// thumbnails and the receipts' mono values without folding them.
    const SHELF_CARD_WIDTH: f32 = 420.0;
    const CARD_INSET: f32 = 18.0;
    /// The card's own padding.
    const CARD_PAD_X: f32 = 14.0;
    /// Acceptance bullets the card keeps — enough to know the claim,
    /// not the whole spec.
    const CARD_BULLETS: usize = 3;

    /// Murmur rows the resting sky keeps — the cell view's own cap
    /// (view 03: two rows, and nothing else).
    const MURMUR_ROWS: usize = 2;
    /// The chrome's margins, view 04's own air.
    const PAD: f32 = 18.0;

    /// The sky for `model`, in `medium`'s lexicon.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let resting = model.sky_at_rest();
        // A bad day (view 14) is the field alone, full-bleed — the
        // volume's chartfield is the body itself: the cards and the
        // murmur foot are the bad day's own composition, and the chrome
        // has nothing to say that the cards do not.
        if model.bad_day().is_some() {
            return Element::surface()
                .grow(1.0)
                .fill(lex.base)
                .child(Element::surface().grow(1.0).id(Self::FIELD));
        }
        let mut column = Element::surface().grow(1.0).fill(lex.base).pad(Self::PAD);
        if !resting {
            let (done, total) = model.rail().progress();
            let mut chrome = Element::row().gap(14.0).center().child(Stat::el(
                &done.to_string(),
                &total.to_string(),
                &lex,
            ));
            if let Some(run) = model.roadmap().ship_run(model.now()) {
                let shipped = model.roadmap().graph_chunk(run.star).map_or_else(
                    || "shipped".to_owned(),
                    |chunk| format!("shipped · {}", chunk.id),
                );
                chrome = chrome.child(SeverityCap::el(Severity::Good, None, &shipped, &lex));
            }
            // The inbox's open count rides the sky's chrome while there is one: the
            // same chip the Signals header wears, so the two rooms cannot count apart.
            if SignalsComposition::open_count(model) > 0 {
                chrome = chrome.child(SignalsComposition::chips(model, medium));
            }
            column = column.child(chrome);
        }
        column = column.child(Element::surface().grow(1.0).id(Self::FIELD));
        if resting {
            column = column.child(murmur(model, &lex));
        }
        // The selected star's card: what the click told you. Floated
        // over the field's corner, never at rest — a selection is
        // chrome, and the resting room has exhaled its chrome.
        if !resting && let Some(chunk) = model.roadmap().selected() {
            // The card's state is the star's: a fault's ring wins, and
            // otherwise the graph's reading, which knows what the chunk
            // stands on — so the word on the card is the word the
            // constellation draws.
            let state = match model.star_state(chunk) {
                StarState::Blocked => StarState::Blocked,
                fallback => model.roadmap().star_state(&chunk.id).unwrap_or(fallback),
            };
            column = column.child(detail(chunk, state, model.shelf(), &lex, medium));
        }
        column
    }
}

/// The selected chunk, told: the status cap with the band and the workstream, the id,
/// the claim, the first acceptance bullets, what it stands on, and the verb the engines
/// answer — \[Start\] while it can start, \[Complete\] while it is in progress. A
/// shipped chunk whose deliverables have arrived carries its shelf — the gallery, the
/// receipts and \[Export\] — on a wider card.
fn detail(
    chunk: &RoadmapChunk,
    state: StarState,
    shelf: Option<&Shelf>,
    lex: &Lexicon,
    medium: Medium,
) -> Element {
    let width = if shelf.is_some() {
        SkyComposition::SHELF_CARD_WIDTH
    } else {
        SkyComposition::CARD_WIDTH
    };
    let inner = width - 2.0 * SkyComposition::CARD_PAD_X;
    let (status_word, status_ink) = status_face(state, lex);
    let mut cap = Element::row()
        .gap(8.0)
        .center()
        .child(
            Element::text(Voice::CAP, status_ink, status_word)
                .claim(Component::Cap)
                .nowrap(),
        )
        .child(
            Element::text(Voice::CAP, lex.sb0, chunk.band())
                .claim(Component::Cap)
                .nowrap(),
        );
    if let Some(group) = &chunk.group {
        cap = cap.child(
            Element::text(Voice::CAP, lex.ov0, group)
                .claim(Component::Cap)
                .nowrap(),
        );
    }
    let mut card = Card::overlay(lex.base, SkyComposition::CARD_PAD_X, 12.0, lex)
        .width(width)
        .pin_right(SkyComposition::CARD_INSET)
        .pin_top(SkyComposition::CARD_INSET)
        .gap(8.0)
        .align_start()
        .child(cap)
        .child(
            Element::text(Voice::VALUE, lex.mauve, &chunk.id)
                .claim(Component::Value)
                .nowrap(),
        )
        .child(
            Element::text(Voice::SLOT, lex.tx, &chunk.title)
                .claim(Component::Prose)
                .width(inner),
        );
    for bullet in chunk.acceptance.iter().take(SkyComposition::CARD_BULLETS) {
        card = card.child(
            Element::text(Voice::HINT, lex.sb1, format!("\u{2022} {bullet}"))
                .claim(Component::Prose)
                .width(inner),
        );
    }
    let more = chunk
        .acceptance
        .len()
        .saturating_sub(SkyComposition::CARD_BULLETS);
    if more > 0 {
        card = card.child(
            Element::text(Voice::CAP, lex.ov0, format!("+{more} more"))
                .claim(Component::Cap)
                .nowrap(),
        );
    }
    if !chunk.deps.is_empty() {
        card = card.child(
            Element::text(
                Voice::CAP,
                lex.sb0,
                format!("stands on {}", chunk.deps.join(" \u{b7} ")),
            )
            .claim(Component::Cap)
            .width(inner),
        );
    }
    let mut primary = None;
    // Controls name chunk ids: the verb says which chunk it starts or completes, the
    // way Birth's `Claim <id>` does.
    if matches!(
        chunk.state,
        magistr_cockpit::cockpit::ChunkState::Pending
            | magistr_cockpit::cockpit::ChunkState::Blocked
            | magistr_cockpit::cockpit::ChunkState::Backlog
    ) {
        primary = Some(Action::new(
            format!("Start {}", chunk.id),
            SkyComposition::START,
        ));
    }
    if chunk.is_in_progress() {
        primary = Some(Action::new(
            format!("Complete {}", chunk.id),
            SkyComposition::COMPLETE,
        ));
    }
    if let Some(primary) = primary {
        card = card.child(ActionRow::new(Some(primary), Vec::new()).el(medium));
    }
    // The shelf carries the card's one lit exit for a shipped chunk:
    // [Export]. A Done chunk offers no Start or Complete, so the count
    // holds.
    if let Some(shelf) = shelf {
        card = card.child(ShelfComposition::el(
            shelf,
            inner,
            Card::overlay_ground(lex.base, lex),
            medium,
        ));
    }
    card
}

/// The status's word and ink — the star vocabulary's own hues, so the card and the
/// constellation cannot disagree about what a state is. The state's word and ink — the
/// one vocabulary in the star vocabulary's own hues, so the card and the constellation
/// cannot disagree about what a state is: the working star's mauve, the alert ring's
/// red, the stuck star's peach, the shipped star's green.
fn status_face(state: StarState, lex: &Lexicon) -> (&'static str, Srgba) {
    let ink = match state {
        StarState::Executing => lex.mauve,
        StarState::Stuck => lex.peach,
        StarState::Blocked => lex.red,
        StarState::Done => lex.green,
        StarState::Ready | StarState::Pending => lex.tx,
        StarState::Ghost => lex.sb0,
    };
    (StateWord::of_star(state).word(), ink)
}

/// The murmur the room keeps at rest: one status row per chunk a
/// session is on, in the star's own state, capped like the cell view's
/// — read, never clicked.
fn murmur(model: &Model, lex: &Lexicon) -> Element {
    let mut rows = Element::surface().align_start();
    let chunks: Vec<_> = model
        .roadmap()
        .rows()
        .into_iter()
        .filter(|chunk| {
            let state = model.star_state(chunk);
            state.breathes() || state.alerts()
        })
        .take(SkyComposition::MURMUR_ROWS)
        .collect();
    for chunk in chunks {
        let state = model.star_state(chunk);
        let (dot, state_ink) = if state.alerts() {
            (lex.red, Some(lex.red))
        } else {
            (lex.mauve, None)
        };
        let metric = model
            .loop_status()
            .filter(|_| model.rail().chunk() == Some(chunk.id.as_str()))
            .map(|status| status.elapsed_text());
        rows = rows.child(StatusRow::el(
            dot,
            &chunk.id,
            (StateWord::of_star(state).word(), state_ink),
            metric.as_deref().unwrap_or(""),
            false,
            lex.base,
            lex,
        ));
    }
    rows
}

#[cfg(test)]
#[path = "comp_sky.tests.rs"]
mod tests;
