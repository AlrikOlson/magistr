//! Escape-key navigation: close overlays, then move up.

use super::cmd::Cmd;
use super::dock_item::DockItem;
use super::model::Model;
use super::place::Place;

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
        if model.condition_open {
            model.condition.take();
            model.condition_open = false;
            return Some(Vec::new());
        }
        if let Some(DockItem::Permission(p)) = model.dock.front() {
            model.stood_down = Some(p.id());
        }
        model.place = Place::Session;
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
    if model.place == Place::Session && model.turn.collapse_expanded() {
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
    if let Some(up) = model.place.up() {
        model.place = up;
    }
    Some(Vec::new())
}

#[cfg(test)]
#[path = "nav.tests.rs"]
mod tests;
