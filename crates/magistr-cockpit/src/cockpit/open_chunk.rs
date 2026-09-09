//! Open a chunk: Enter navigates to it, and "open a
//! chunk" means one thing from every opener.

use super::dock_item::DockItem;
use super::model::Model;
use super::place::Place;
use super::stage_tab::StageTab;
use super::star_state::StarState;

/// Open the chunk `id`: the deep link every opener takes — the ⌘K
/// palette's ↵, a star on the Sky, the Brief's verbs, the Trace's chunk
/// jump, and whatever opens a chunk later — so "open a chunk" lands the
/// same place from all of them. `false` when the roadmap has no such
/// chunk, and then nothing moves.
///
/// The chunk is selected first, on both surfaces: the Sky's card and
/// the star's framing follow the selection. A terminal has no Place and
/// keeps what it always did — the stage turns to the Roadmap tab. A
/// window decides the Place by the chunk's status, in the sky's own
/// vocabulary ([`StarState`]) so the two cannot disagree:
///
/// - **Executing** → its Session, when the rail names this chunk (that
///   is what makes the session *its*); an executing chunk this window
///   has no session on gets the Sky card like any other star.
/// - **Stuck** → its decision: the Session, with a Moment that was stood
///   down to the dock raised again — unanswered, never an answer. With
///   no decision on this window's dock there is nothing here to open,
///   and the Sky card shows the ringed star.
/// - **Blocked** → the Sky card, which carries the blocker and its Clear
///   verb.
/// - **Done** → its Report (view 09): the chapter and the deliverables
///   shelf, at altitude 3. The shelf itself is asked for by the opener
///   (`shelf_collect` in `update`), since a fold answers no `Cmd`.
/// - anything else → the Sky's selection card.
///
/// ⌘↵ on a palette row is the one opener that does not come here by
/// design: it acts on the row and the camera holds.
pub fn open_chunk(model: &mut Model, id: &str) -> bool {
    let (now, animate) = (model.now(), model.theme.motion().is_full());
    if !model.roadmap.select(id, now, animate) {
        return false;
    }
    if !model.surface().is_native() {
        model.show(StageTab::Roadmap);
        return true;
    }
    model.place = destination(model, id);
    true
}

/// The status → Place mapping, on a chunk the roadmap has.
fn destination(model: &mut Model, id: &str) -> Place {
    let state = model.roadmap.chunk(id).map_or(StarState::Pending, |chunk| {
        StarState::of(&chunk.status, false)
    });
    match state {
        StarState::Executing if model.rail.chunk() == Some(id) => Place::Session,
        StarState::Stuck if matches!(model.dock.front(), Some(DockItem::Permission(_))) => {
            model.stood_down = None;
            Place::Session
        }
        StarState::Done => Place::Report,
        _ => Place::Sky,
    }
}

#[cfg(test)]
#[path = "open_chunk.tests.rs"]
mod tests;
