//! Navigation owns shared view and record destinations. Native overlays keep
//! their own escape order, without replacing selection or session state.

use super::cmd::Cmd;
use super::dock_item::DockItem;
use super::model::Model;
use super::place::Place;
use super::{Selected, StageTab};

/// Open the existing record by identity. Missing records retain their identity
/// so the details can explain unavailability rather than showing another row.
pub(super) fn open_record(model: &mut Model, selected: Selected) {
    let tab = match &selected {
        Selected::Chunk(_) | Selected::Signal(_) => StageTab::Roadmap,
        Selected::Tool(_) => StageTab::Turn,
        _ => StageTab::Session,
    };
    model.show(tab);
    if matches!(selected, Selected::Signal(_)) {
        model.show_place(Place::Signals);
    }
    model.inspect(selected);
    super::update::reveal_session_selection(model);
}

/// Esc on a window, in the navigation order. The topmost overlay
/// goes first — the Brief here; the ⌘K palette, the accelerator sheet
/// and the completion popup are shared with the terminal and close on
/// their own before this fold on either surface. Then a view-local
/// viewer pane (the Changes journal's file, the Trace feed's report).
/// Then a Moment: a condition being written is cleared first, the way
/// the card's own keys clear it, and otherwise the card *stands down*
/// to the Needs-you dock unanswered and the camera returns to the
/// Session. Then one altitude up.
///
/// `None` on a terminal, whose Esc keeps every meaning it had. `Some`
/// on a window always: Esc is navigation there, and is spent even at
/// the Galaxy, which holds — a window's Esc never answers a permission
/// and never aborts a paused loop.
pub fn esc(model: &mut Model) -> Option<Vec<Cmd>> {
    if !model.surface().is_native() {
        return None;
    }
    if model.brief().is_some() && model.dismiss_brief() {
        return Some(Vec::new());
    }
    let viewer_closed = match model.place {
        Place::Changes => model.changes.close_viewer(),
        Place::Trace => model.trace.close_viewer(),
        _ => false,
    };
    if viewer_closed {
        return Some(Vec::new());
    }
    if model.moment().is_some() || model.review().is_some() {
        if model.decision.clear_condition() {
            return Some(Vec::new());
        }
        if let Some(DockItem::Permission(p)) = model.dock.front() {
            model.decision.stand_down(p.id());
        }
        model.show(StageTab::Turn);
        return Some(Vec::new());
    }
    // The terminal dock lowers ahead of any altitude move — it stands over the
    // Session's feed, so it is the first thing Esc peels there.
    if model.place == Place::Session && model.lower_term_dock() {
        return Some(Vec::new());
    }
    // A card open on the Session's feed folds before the camera moves:
    // collapse is not navigation, so it goes ahead
    // of it — the newest open card first, one per press.
    if model.place == Place::Session && model.session.turn.collapse_expanded() {
        return Some(Vec::new());
    }
    // Birth: a field being typed closes
    // first; a later moment stands the flow down to the ground with
    // the survey kept; the ground itself rises to the Galaxy.
    if model.place == Place::Birth
        && (model.birth.close_field() || model.birth.cancel_edit() || model.birth.stand_down())
    {
        return Some(Vec::new());
    }
    if model.place == Place::Sky {
        if model.roadmap.is_track_search_editing() {
            super::work::browse_control(model, super::Widget::RoadmapSearchClear);
            return Some(Vec::new());
        }
        if model.pane == super::Pane::Inspector {
            model.pane = super::Pane::Stage;
            return Some(Vec::new());
        }
    }
    if let Some(up) = model.place.up() {
        model.show_place(up);
    }
    Some(Vec::new())
}

#[cfg(test)]
#[path = "nav.tests.rs"]
mod tests;
