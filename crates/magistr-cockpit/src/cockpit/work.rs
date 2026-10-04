//! Work object interaction — the one owner of how a roadmap chunk is
//! selected, opened and acted on from the screen.
//!
//! These folds live together because they encode one contract and
//! change together: which chunk is selected (by id, never by row — a
//! row, a star index or a graph position is a hit target the next paint
//! can renumber, and is resolved to a chunk at press time); where
//! opening a chunk lands ([`open_chunk`], the deep link every opener
//! takes, and [`rise_to_chunk`], the identity chip's rise to the star);
//! how the list is filtered and sorted and the tech-tree zoomed, panned
//! and hovered; and the verbs a chunk offers — Start and Complete on its
//! card, Accept, Reject and Clear on the dock's proposal and blocker
//! cards. Every verb is an ask for the engine port (`Cmd::Engine`): the
//! model changes only when the port answers, and a refusal restores the
//! dock's answer (`EngineEvent::PortFailed`, folded in `update`).
//!
//! The roadmap's own state — the chunks, the selection, the camera —
//! stays in [`Roadmap`](super::Roadmap); this module is the interaction
//! over it.

use super::cmd::Cmd;
use super::dock_item::DockItem;
use super::engine_cmd::EngineCmd;
use super::model::Model;
use super::pane::Pane;
use super::place::Place;
use super::roadmap_sort::RoadmapSort;
use super::selected::Selected;
use super::shelf_cmd::ShelfCmd;
use super::stage_tab::StageTab;
use super::surface::Surface;
use super::update::{ZOOM_KEY_STEP, ZOOM_NOTCH};
use super::widget::Widget;
use crate::input::{KeyCode, KeyEvent, MouseEvent};
use crate::turn::Viewport;
use crate::widget_id::WidgetId;

/// Open a chunk by stable identity on the Roadmap, on either surface.
/// Missing chunks leave navigation unchanged. A blocked decision may be
/// recalled, but opening work never answers it. Reports are explicit destinations.
pub(super) fn open_chunk(model: &mut Model, id: &str) -> bool {
    if !inspect_chunk(model, id) {
        return false;
    }
    model.show(StageTab::Roadmap);
    model.pane = Pane::Inspector;
    true
}

/// Select the shared work details without changing the surrounding view.
fn inspect_chunk(model: &mut Model, id: &str) -> bool {
    let (now, animate) = glide(model);
    if !model.roadmap.select(id, now, animate) {
        return false;
    }
    model.inspect(Selected::Chunk(id.to_owned()));
    model.roadmap_requests_open = false;
    if model
        .roadmap
        .chunk(id)
        .is_some_and(|chunk| chunk.state == super::ChunkState::Stuck)
        && matches!(model.dock.front(), Some(DockItem::Permission(_)))
    {
        model.decision.recall();
    }
    true
}

/// Reports are explicit destinations; selecting completed work opens its details.
pub(super) fn open_report(model: &mut Model) -> Vec<Cmd> {
    if model
        .roadmap
        .selected()
        .is_none_or(|chunk| !chunk.state.is_done())
    {
        return Vec::new();
    }
    model.show_place(Place::Report);
    shelf_collect(model)
}

/// Focus a chunk's roadmap node without recalling a decision.
pub(super) fn rise_to_chunk(model: &mut Model, id: &str) -> bool {
    let (now, animate) = glide(model);
    if !model.roadmap.select(id, now, animate) {
        return false;
    }
    model.inspect(Selected::Chunk(id.to_owned()));
    model.roadmap_requests_open = false;
    model.show(StageTab::Roadmap);
    true
}

/// Focus the work named by the session. A missing identity leaves it unchanged.
pub(super) fn session_chunk_click(model: &mut Model) {
    let Some(id) = model.rail.chunk().map(str::to_owned) else {
        return;
    };
    rise_to_chunk(model, &id);
}

/// Resolve a graph hit through the shared chunk opener.
///
/// The chunk is looked up by the graph index the star's id carries
/// rather than remembered from the paint, so a roadmap re-seeded
/// between the frame and the press opens whatever is at that place
/// now, or nothing.
pub(super) fn sky_star_click(model: &mut Model, index: usize) -> Vec<Cmd> {
    let Some(id) = model
        .roadmap()
        .graph_chunk(index)
        .map(|chunk| chunk.id.clone())
    else {
        return Vec::new();
    };
    if model.uses_flight_recorder() && model.tab() != StageTab::Roadmap {
        inspect_chunk(model, &id);
    } else {
        open_chunk(model, &id);
    }
    shelf_collect(model)
}

/// The chunk jump on an expanded step's execution ref: the chunk the
/// ref names is opened through the one open-chunk fold — on a terminal
/// the stage turns to the Roadmap tab with it selected and the
/// tech-tree's camera brings its node into frame.
///
/// The chunk is resolved here rather than carried in the id, so the
/// click re-runs exactly the resolution the view painted the link from.
/// A ref no chunk answers is not painted as a link at all, so this
/// cannot normally arrive with one — and if it does (a roadmap re-seeded
/// between the paint and the press), nothing moves rather than the stage
/// turning to a tab with nothing to show.
pub(super) fn chunk_jump(model: &mut Model, number: u32) -> Vec<Cmd> {
    let Some(id) = model
        .trace()
        .step(number)
        .and_then(|step| step.execution_ref.as_deref())
        .and_then(|reference| model.roadmap().chunk_for_ref(reference))
        .map(|chunk| chunk.id.clone())
    else {
        return Vec::new();
    };
    open_chunk(model, &id);
    if model.uses_flight_recorder() {
        model.pane = Pane::Inspector;
    }
    shelf_collect(model)
}

/// Select the chunk at a visible list row by stable identity.
pub(super) fn row_click(model: &mut Model, index: usize) {
    if let Some(chunk) = model.roadmap.rows().get(index) {
        model.roadmap.browse.focus = Some(super::TrackFocus::Item(chunk.id.clone()));
        activate_track_focus(model);
        reveal_track_focus(model);
    }
}

/// A press on the tech-tree: the node under the pointer (if any) is
/// selected by id, and a drag begins at the pointer so the pan that may
/// follow moves by the pointer's exact delta.
pub(super) fn graph_click(model: &mut Model, event: &MouseEvent) {
    let Some((x, y)) = graph_cell(model, event) else {
        return;
    };
    if model.roadmap.click_at(x, y, model.now())
        && let Some(chunk) = model.roadmap.selected()
    {
        model.inspect(Selected::Chunk(chunk.id.clone()));
        model.pane = Pane::Stage;
    }
    model.roadmap_drag = Some(graph_pointer(model, event));
}

/// The pointer moving over the tech-tree or a sky star: the nearest
/// node becomes the tooltip's chunk; past every node it clears.
pub(super) fn graph_hover(model: &mut Model, target: Option<WidgetId>, event: &MouseEvent) {
    if (target == Some(Widget::RoadmapGraph.id())
        || target.and_then(Widget::sky_star_index).is_some())
        && let Some((x, y)) = graph_cell(model, event)
    {
        model.roadmap.hover_at(x, y, model.now());
    }
}

/// A drag that began on the tech-tree: the pan moves by the pointer's
/// exact delta — whole cells on a terminal, fractional ones on a window
/// ([`Model::pointer_at`]). Nothing moves when no drag began there.
pub(super) fn graph_drag(model: &mut Model, event: &MouseEvent) {
    if let Some((last_x, last_y)) = model.roadmap_drag {
        let (x, y) = graph_pointer(model, event);
        model.roadmap.pan_by(x - last_x, y - last_y);
        model.roadmap_drag = Some((x, y));
    }
}

/// The button came up: the tech-tree's drag, if one began, ends.
pub(super) fn end_graph_drag(model: &mut Model) {
    model.roadmap_drag = None;
}

/// A wheel notch over the tech-tree zooms it about the pointer instead
/// of scrolling the list under it; whether the tree took the notch. In
/// cell-only mode the runtime reports no rect, so the wheel scrolls
/// there as everywhere.
pub(super) fn graph_wheel(model: &mut Model, event: &MouseEvent, zoom_in: bool) -> bool {
    let Some((x, y)) = graph_cell(model, event) else {
        return false;
    };
    let factor = if zoom_in {
        ZOOM_NOTCH
    } else {
        1.0 / ZOOM_NOTCH
    };
    model.roadmap.zoom_at(factor, x, y);
    true
}

/// A zoom accelerator on the Roadmap tab with an empty prompt: `+`
/// (or an unshifted `=`) steps the tech-tree in, `-` steps it out and
/// `0` puts it back to the fit — the pan with it.
///
/// Gated on there being a camera at all, and gated *visibly*: the
/// `Roadmap` verbs answer whether they moved anything, and a `false`
/// falls through so the key stays the prompt's letter. A cell-only
/// terminal draws the tech-tree as a legend rather than a picture,
/// has no camera, and so keeps `-` and `0` for typing.
pub(super) fn roadmap_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    if model.shown_tab() != StageTab::Roadmap
        || !model.session.prompt.is_empty()
        || !key.mods.is_empty()
    {
        return None;
    }
    let (now, animate) = glide(model);
    let moved = match key.code {
        KeyCode::Char('+' | '=') => model.roadmap.zoom_step(ZOOM_KEY_STEP, now, animate),
        KeyCode::Char('-') => model.roadmap.zoom_step(1.0 / ZOOM_KEY_STEP, now, animate),
        KeyCode::Char('0') => model.roadmap.reset_view(now, animate),
        _ => return None,
    };
    moved.then(Vec::new)
}

/// The list's filter chip: the status filter cycles and the list goes
/// back to its top.
pub(super) fn filter_click(model: &mut Model) {
    model.roadmap.cycle_filter();
    model.roadmap_viewport = Viewport::at_top();
    model.pane = Pane::Stage;
}

/// The column a sort header widget names, when it is one.
pub(super) fn sort_column(widget: Widget) -> Option<RoadmapSort> {
    RoadmapSort::ALL
        .into_iter()
        .find(|column| column.widget() == widget)
}

/// A sort header: the list sorts by that column (a repeat reverses it)
/// and goes back to its top.
pub(super) fn sort_click(model: &mut Model, column: RoadmapSort) {
    model.roadmap.sort_by(column);
    model.roadmap_viewport = Viewport::at_top();
    model.pane = Pane::Stage;
}

/// The tree's reset verb: the camera glides back to the fit.
pub(super) fn reset_click(model: &mut Model) {
    let (now, animate) = glide(model);
    model.roadmap.reset_view(now, animate);
}

/// \[Start\] or \[Complete\] on the selected chunk's detail card — an
/// ask for the engine port; without a selection there is nothing to act
/// on.
pub(super) fn roadmap_chunk_click(model: &Model, complete: bool) -> Vec<Cmd> {
    let Some(chunk) = model.roadmap.selected() else {
        return Vec::new();
    };
    let allowed = if complete {
        model.roadmap.can_complete(chunk)
    } else {
        model.roadmap.can_start(chunk)
    };
    if !allowed {
        return Vec::new();
    }
    let id = chunk.id.clone();
    vec![Cmd::Engine(if complete {
        EngineCmd::CompleteChunk { id }
    } else {
        EngineCmd::StartChunk { id }
    })]
}

/// \[Accept\] or \[Reject\] on the Roadmap tab's proposal at `index`.
pub(super) fn roadmap_proposal_click(model: &Model, index: usize, accept: bool) -> Vec<Cmd> {
    let proposals = model.roadmap.proposals();
    let Some((chunk, kind, ..)) = proposals.get(index) else {
        return Vec::new();
    };
    vec![Cmd::Engine(EngineCmd::ResolveProposal {
        chunk: chunk.id.clone(),
        kind: *kind,
        accept,
    })]
}

/// A click on the front proposal's Accept or Reject button — an ask for
/// the engine port; a click while another card is front answers nothing.
pub(super) fn proposal_click(model: &mut Model, accept: bool) -> Vec<Cmd> {
    let cmd = match model.dock.front() {
        Some(DockItem::Proposal { chunk, kind, .. }) => Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: chunk.clone(),
            kind: *kind,
            accept,
        }),
        _ => return Vec::new(),
    };
    if let Cmd::Engine(answer) = &cmd {
        model.dock.answer_front(answer);
    }
    vec![cmd]
}

/// A click on the front blocker's Clear button.
pub(super) fn blocker_click(model: &mut Model) -> Vec<Cmd> {
    let cmd = match model.dock.front() {
        Some(DockItem::Blocker { chunk, .. }) => Cmd::Engine(EngineCmd::ClearBlocker {
            chunk: chunk.clone(),
        }),
        _ => return Vec::new(),
    };
    if let Cmd::Engine(answer) = &cmd {
        model.dock.answer_front(answer);
    }
    vec![cmd]
}

/// A shipped chunk opened on a window asks for its deliverables:
/// the shelf is gathered by the entry point's job and
/// lands as [`JobOutput::Shelf`](crate::job::JobOutput::Shelf). Nothing
/// is asked on a terminal — it has no gallery to show one — nor for a
/// chunk that is not Done, nor while the model already holds that
/// chunk's shelf.
pub(super) fn shelf_collect(model: &Model) -> Vec<Cmd> {
    if model.surface() != Surface::Native {
        return Vec::new();
    }
    let Some(chunk) = model.roadmap.selected().filter(|c| c.state.is_done()) else {
        return Vec::new();
    };
    if model.shelf().is_some() {
        return Vec::new();
    }
    vec![Cmd::Shelf(ShelfCmd::Collect {
        chunk: chunk.id.clone(),
    })]
}

/// A mouse position as block-relative units on the tech-tree, in its
/// camera's units: pixels on a window, cells on a terminal. The
/// position inside the block is [`Model::pointer_at`]'s: the cell's
/// centre on a terminal, the exact pixel's cell on a window.
pub(super) fn graph_pointer(model: &Model, event: &MouseEvent) -> (f32, f32) {
    let (x, y) = model.pointer_at(event);
    if model.roadmap_graph_px().is_some() {
        let cell = model.cell_size();
        (x * f32::from(cell.width), y * f32::from(cell.height))
    } else {
        (x, y)
    }
}

/// The pointer as block-relative units on the tech-tree, through the
/// rect the runtime reported after layout; `None` while the graph is
/// not on screen or the pointer sits outside it — a click on the
/// legend gutter beside the graph must never take a node.
pub(super) fn graph_cell(model: &Model, event: &MouseEvent) -> Option<(f32, f32)> {
    if let Some(rect) = model.roadmap_graph_px() {
        let (x, y) = graph_pointer(model, event);
        return (x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height)
            .then_some((x - rect.x, y - rect.y));
    }
    let rect = model.roadmap_graph_rect?;
    let inside = event.col >= rect.x
        && event.col < rect.x.saturating_add(rect.width)
        && event.row >= rect.y
        && event.row < rect.y.saturating_add(rect.height);
    if !inside {
        return None;
    }
    let (x, y) = model.pointer_at(event);
    Some((x - f32::from(rect.x), y - f32::from(rect.y)))
}

/// The step every glide starts at and whether it animates at all.
fn glide(model: &Model) -> (f32, bool) {
    (model.now(), model.theme.motion().is_full())
}

pub(super) fn signal_click(model: &mut Model, index: usize) {
    if let Some(signal) = model.roadmap.signals().get(index) {
        model.roadmap.browse.focus = Some(super::TrackFocus::Signal(signal.id.clone()));
        activate_track_focus(model);
        reveal_track_focus(model);
    }
}

/// Keep a scrolled track list anchored to the same record across live updates.
pub(super) fn browse_anchor(model: &Model) -> Option<super::TrackFocus> {
    model
        .roadmap
        .track_rows()
        .get(model.roadmap_top())
        .map(super::TrackRow::focus)
}

pub(super) fn restore_browse_anchor(model: &mut Model, anchor: Option<super::TrackFocus>) {
    if let Some(anchor) = anchor
        && let Some(row) = model
            .roadmap
            .track_rows()
            .iter()
            .position(|row| row.focus() == anchor)
    {
        let (total, height) = model.roadmap_window();
        let top = model.roadmap_top();
        model.roadmap_viewport.scroll_by(
            row as isize - top as isize,
            total,
            height,
            model.now(),
            false,
        );
    }
}

pub(super) fn track_click(model: &mut Model, index: usize) {
    model.roadmap.focus_track_row(index);
    model.roadmap.toggle_focused_track();
    model.pane = Pane::Stage;
}

fn activate_track_focus(model: &mut Model) {
    match model.roadmap.track_focus().cloned() {
        Some(super::TrackFocus::Track(_)) => model.roadmap.toggle_focused_track(),
        Some(super::TrackFocus::Item(id)) => {
            inspect_chunk(model, &id);
            model.pane = Pane::Inspector;
        }
        Some(super::TrackFocus::Signal(id)) => {
            model.inspect(Selected::Signal(id));
            model.pane = Pane::Inspector;
        }
        None => {}
    }
}

pub(super) fn browse_control(model: &mut Model, widget: Widget) {
    match widget {
        Widget::RoadmapTracks => {
            model.roadmap.browse.dependencies = false;
            model.pane = Pane::Stage;
        }
        Widget::RoadmapDependencies => {
            model.roadmap.browse.dependencies = true;
            model.pane = Pane::Stage;
        }
        Widget::RoadmapTrackFilter => {
            model.roadmap.cycle_track_filter();
            model.roadmap_viewport = Viewport::at_top();
        }
        Widget::RoadmapSearch => {
            model.roadmap.browse.editing = true;
        }
        Widget::RoadmapSearchClear => {
            search_query(model, String::new());
            model.roadmap.browse.editing = false;
        }
        Widget::RoadmapReveal => {
            search_query(model, String::new());
            model.roadmap.browse.editing = false;
            model.roadmap.reveal_selected();
            reveal_track_focus(model);
            model.pane = Pane::Stage;
        }
        _ => {}
    }
}

pub(super) fn search_query(model: &mut Model, query: String) {
    if model.roadmap.track_query().is_empty() && !query.is_empty() {
        model.roadmap.browse.saved_scroll = Some(model.roadmap_viewport);
        model.roadmap_viewport = Viewport::at_top();
    }
    let cleared = !model.roadmap.track_query().is_empty() && query.is_empty();
    model.roadmap.set_track_query(query);
    if cleared && let Some(viewport) = model.roadmap.browse.saved_scroll.take() {
        model.roadmap_viewport = viewport;
    }
}

fn reveal_track_focus(model: &mut Model) {
    if let Some(row) = model.roadmap.focused_track_index() {
        let (total, height) = model.roadmap_window();
        if height == 0 {
            return;
        }
        let top = model.roadmap_top();
        if row < top || row >= top.saturating_add(height) {
            let next = if row < top {
                row
            } else {
                row.saturating_add(1).saturating_sub(height)
            };
            model.roadmap_viewport.scroll_by(
                next as isize - top as isize,
                total,
                height,
                model.now(),
                false,
            );
        }
    }
}

/// Browsing receives input only while its list or search field has focus.
pub(super) fn track_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    use crate::input::Modifiers;
    if model.shown_tab() != StageTab::Roadmap || model.palette().is_some() || model.is_launcher() {
        return None;
    }
    if key.mods == Modifiers::ALT {
        let widget = match key.code {
            KeyCode::Char('/') => Widget::RoadmapSearch,
            KeyCode::Char('t') => Widget::RoadmapTrackFilter,
            KeyCode::Char('d') => {
                if model.roadmap.is_dependencies_view() {
                    Widget::RoadmapTracks
                } else {
                    Widget::RoadmapDependencies
                }
            }
            KeyCode::Char('v') => Widget::RoadmapReveal,
            _ => return None,
        };
        browse_control(model, widget);
        return Some(Vec::new());
    }
    if model.roadmap.is_track_search_editing() {
        match key.code {
            KeyCode::Char(c) if key.mods.is_empty() || key.mods == Modifiers::SHIFT => {
                let mut query = model.roadmap.track_query().to_owned();
                query.push(c);
                search_query(model, query);
            }
            KeyCode::Backspace => {
                let mut query = model.roadmap.track_query().to_owned();
                query.pop();
                search_query(model, query);
            }
            KeyCode::Esc => {
                search_query(model, String::new());
                model.roadmap.browse.editing = false;
            }
            KeyCode::Enter | KeyCode::Down => {
                model.roadmap.browse.editing = false;
                model.pane = Pane::Stage;
                model.roadmap.move_track_focus(1);
            }
            _ => {}
        }
        return Some(Vec::new());
    }
    if model.pane != Pane::Stage || !key.mods.is_empty() || !model.session.prompt.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Up => model.roadmap.move_track_focus(-1),
        KeyCode::Down => model.roadmap.move_track_focus(1),
        KeyCode::Home => model.roadmap.move_track_focus(isize::MIN),
        KeyCode::End => model.roadmap.move_track_focus(isize::MAX),
        KeyCode::Left => model.roadmap.branch_track_focus(false),
        KeyCode::Right => model.roadmap.branch_track_focus(true),
        KeyCode::Enter | KeyCode::Char(' ') if model.roadmap.track_focus().is_some() => {
            activate_track_focus(model)
        }
        _ => return None,
    }
    reveal_track_focus(model);
    Some(Vec::new())
}

#[cfg(test)]
#[path = "work.tests.rs"]
mod tests;
