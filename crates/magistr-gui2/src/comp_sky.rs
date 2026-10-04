use magistr_cockpit::cockpit::{Model, StateWord};
use magistr_cockpit::theme::Severity;

use crate::comp_signals::SignalsComposition;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::severity_cap::SeverityCap;
use crate::stat::Stat;
use crate::status_row::StatusRow;

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

    /// The detail card's measure, and where it floats off the field's
    /// top-right corner.
    const CARD_WIDTH: f32 = 300.0;
    const CARD_INSET: f32 = 18.0;
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
        if !resting && model.selected().is_some() {
            let height = (f32::from(model.size().rows) * f32::from(model.cell_size().height)
                - 120.0)
                .max(1.0);
            column = column.child(
                crate::comp_workspace_details::DetailsComposition::el(model, height, &lex)
                    .width(Self::CARD_WIDTH)
                    .pin_right(Self::CARD_INSET)
                    .pin_top(Self::CARD_INSET),
            );
        }
        column
    }
}

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
