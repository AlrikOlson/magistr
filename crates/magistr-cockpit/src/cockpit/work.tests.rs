use magistr_domain::policy::RiskTier;

use super::*;
use crate::cockpit::{Msg, Size, StageTab, Surface, update};
use crate::engine::EngineEvent;
use crate::input::{KeyCode, KeyEvent};
use magistr_application::sessions::{AgentEvent, AskPlan, PermissionChoice, PermissionKind};

/// A store with `here` in progress, `stuck` stuck, `shipped` done,
/// `blocked` blocked and `next` pending — and, when `other` is set, a
/// second chunk in progress ahead of `here`, so the rail (which names
/// the store's first chunk in progress) is on that one and `here` is a
/// session this window is not on.
fn seeded(surface: Surface, other: Option<&str>) -> Model {
    let mut m = Model::new(Size::new(120, 40)).with_surface(surface);
    let chunk = |id: &str, status: &str| serde_json::json!({"id": id, "name": id, "title": id, "status": status, "priority": 200});
    let mut chunks = vec![
        chunk("here", "in_progress"),
        chunk("stuck", "stuck"),
        chunk("shipped", "done"),
        chunk("blocked", "blocked"),
        chunk("next", "pending"),
    ];
    if let Some(other) = other {
        chunks.insert(0, chunk(other, "in_progress"));
    }
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                progress: crate::harness::fixtures::progress(1, 5),
                chunks: crate::harness::fixtures::chunks_from(chunks),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    m
}

/// A window whose rail is on `here`.
fn window() -> Model {
    seeded(Surface::Native, None)
}

#[test]
fn unavailable_work_actions_do_not_dispatch_and_valid_actions_remain_requests() {
    for surface in [Surface::Terminal, Surface::Native] {
        let mut model = seeded(surface, None);
        for id in ["shipped", "blocked", "stuck"] {
            assert!(open_chunk(&mut model, id));
            assert!(
                roadmap_chunk_click(&model, false).is_empty(),
                "{id} cannot start"
            );
            assert!(
                roadmap_chunk_click(&model, true).is_empty(),
                "{id} cannot complete"
            );
        }
        assert!(open_chunk(&mut model, "next"));
        assert_eq!(
            roadmap_chunk_click(&model, false),
            vec![Cmd::Engine(EngineCmd::StartChunk { id: "next".into() })]
        );
        assert_eq!(
            model.roadmap().chunk("next").unwrap().state,
            super::super::ChunkState::Pending
        );
        assert!(open_chunk(&mut model, "here"));
        assert_eq!(
            roadmap_chunk_click(&model, true),
            vec![Cmd::Engine(EngineCmd::CompleteChunk { id: "here".into() })]
        );
    }
}

fn selected(model: &Model) -> Option<&str> {
    model.roadmap().selected().map(|chunk| chunk.id.as_str())
}

/// A plan-carrying ask: the Moment's card.
fn ask(id: u64) -> Msg {
    Msg::Agent(AgentEvent::PermissionRequest {
        id,
        title: format!("ask {id}"),
        choices: vec![
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
        ],
        edit: false,
        risk: RiskTier::Medium,
        plan: Some(AskPlan::new(["one step"])),
    })
}

// ── open_chunk ──────────────────────────────────────────────────────────

/// A terminal has no Place: opening a chunk there is what it always
/// was — the chunk selected and the stage turned to the Roadmap tab,
/// whatever the chunk's state.
#[test]
fn a_terminal_selects_the_chunk_and_turns_to_the_roadmap_tab() {
    for id in ["here", "stuck", "shipped", "blocked", "next"] {
        let mut t = seeded(Surface::Terminal, None);
        assert!(open_chunk(&mut t, id), "{id}");
        assert_eq!(t.tab(), StageTab::Roadmap, "{id}");
        assert_eq!(t.place(), None, "{id}: a terminal has no place");
        assert_eq!(selected(&t), Some(id));
    }
}

/// A chunk the roadmap does not have opens nothing: the camera holds
/// and no card opens.
#[test]
fn an_unknown_chunk_moves_nothing() {
    let mut w = window().with_place(Place::Trace);
    assert!(!open_chunk(&mut w, "nope"));
    assert_eq!(w.place(), Some(Place::Trace), "the camera held");
    assert_eq!(selected(&w), None);
    assert_eq!(
        w.tab(),
        StageTab::Session,
        "and the shared view is untouched"
    );
}

/// Opening active work selects its Roadmap details, including the current work.
#[test]
fn an_executing_chunk_opens_its_roadmap_details() {
    let mut w = window().with_place(Place::Sky);
    assert!(open_chunk(&mut w, "here"));
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(
        selected(&w),
        Some("here"),
        "and the star is framed under it"
    );
}

/// An executing chunk this window has no session on has no Session to
/// open: it gets the Sky's selection card, like any other star. The
/// rail names the store's first chunk in progress, so a second one in
/// progress is the one the rail is not on.
#[test]
fn an_executing_chunk_without_a_session_here_opens_the_sky_card() {
    let mut other = seeded(Surface::Native, Some("other")).with_place(Place::Session);
    assert_eq!(other.rail().chunk(), Some("other"));
    assert!(open_chunk(&mut other, "here"));
    assert_eq!(
        other.place(),
        Some(Place::Sky),
        "the rail is on another chunk"
    );
    assert_eq!(selected(&other), Some("here"));
}

/// Stuck → its decision: the Session, with the Moment that was stood
/// down to the dock raised again — the card comes back, unanswered.
#[test]
fn a_stuck_chunk_opens_its_decision() {
    let mut w = window().with_place(Place::Sky);
    update(&mut w, ask(7));
    assert!(w.moment().is_some(), "the card came");
    assert_eq!(
        update(&mut w, Msg::Key(KeyEvent::new(KeyCode::Esc))),
        vec![]
    );
    assert!(w.moment().is_none(), "Esc stood it down");
    assert_eq!(w.place(), Some(Place::Session));
    w.place = Place::Trace;

    assert!(open_chunk(&mut w, "stuck"));
    assert_eq!(
        w.place(),
        Some(Place::Sky),
        "the decision opens over the selected work"
    );
    assert!(w.moment().is_some(), "and the card is raised again");
    assert_eq!(w.dock().len(), 1, "raised, never answered");
    assert_eq!(selected(&w), Some("stuck"));
}

/// A stuck chunk whose decision is not on this window's dock has no
/// decision here to open: the Sky card, where the star wears its ring.
#[test]
fn a_stuck_chunk_with_no_decision_here_opens_the_sky_card() {
    let mut w = window().with_place(Place::Session);
    assert!(open_chunk(&mut w, "stuck"));
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(selected(&w), Some("stuck"));
}

/// Blocked → the Sky card, which carries the blocker and its Clear
/// verb; a blocker is not a dock decision.
#[test]
fn a_blocked_chunk_opens_the_sky_card_where_its_blocker_is_cleared() {
    let mut w = window().with_place(Place::Session);
    update(&mut w, ask(7));
    assert!(open_chunk(&mut w, "blocked"));
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(selected(&w), Some("blocked"));
}

/// Completed work opens its details; Report is a separate explicit destination.
#[test]
fn a_done_chunk_opens_details_and_its_report_is_explicit() {
    for from in [Place::Changes, Place::Galaxy, Place::Trace, Place::Sky] {
        let mut w = window().with_place(from);
        assert!(open_chunk(&mut w, "shipped"));
        assert_eq!(w.place(), Some(Place::Sky), "from {from:?}");
        open_report(&mut w);
        assert_eq!(
            w.place(),
            Some(Place::Report),
            "explicit report from {from:?}"
        );
        assert_eq!(selected(&w), Some("shipped"));
    }
}

/// Anything else — pending, ready — is a star with nothing to dive
/// into yet: the Sky's selection card.
#[test]
fn a_pending_chunk_opens_the_sky_card() {
    let mut w = window().with_place(Place::Galaxy);
    assert!(open_chunk(&mut w, "next"));
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(selected(&w), Some("next"));
}

/// The fold is the only site that derives a destination from a chunk:
/// every opener calls it, and nothing outside this module selects on
/// the roadmap by itself — a row, a star or a graph position is
/// resolved to an id here and nowhere else.
#[test]
fn every_opener_routes_through_the_fold_and_selection_stays_here() {
    let update_src = include_str!("update.rs");
    assert_eq!(
        update_src.matches("roadmap.select(").count(),
        0,
        "update.rs must open chunks through open_chunk, never select by itself"
    );
    assert_eq!(
        update_src.matches("open_chunk(model, ").count(),
        3,
        "the palette's ↵, the Brief's verbs and a search hit's ↵"
    );
    let work_src = include_str!("work.rs");
    assert_eq!(
        work_src.matches("roadmap.select(").count(),
        2,
        "open_chunk and rise_to_chunk"
    );
    assert_eq!(
        work_src.matches("open_chunk(model, ").count(),
        2,
        "a sky star and the trace's chunk jump"
    );
    assert_eq!(
        update_src.matches("toggle_row(").count()
            + update_src.matches("click_at(").count()
            + update_src.matches("hover_at(").count(),
        0,
        "rows, stars and graph positions resolve to chunks in work.rs"
    );
}

// ── rise_to_chunk ───────────────────────────────────────────────────────

/// The rise is the same for every state — the executing chunk this
/// session is on, which the open-chunk fold would route back to the
/// Session, rises to the Sky like a pending or a done one: the chip's
/// point is the star, framed.
#[test]
fn every_chunk_rises_to_the_sky_framed() {
    for id in ["here", "next", "shipped"] {
        let mut w = window().with_place(Place::Session);
        assert!(rise_to_chunk(&mut w, id), "{id}");
        assert_eq!(w.place(), Some(Place::Sky), "{id} rises to the Sky");
        assert_eq!(selected(&w), Some(id), "{id} is framed");
    }
}

/// A chunk the roadmap does not hold moves nothing: the camera stays at
/// the Session and nothing is selected.
#[test]
fn an_unknown_chunk_does_not_rise() {
    let mut w = window().with_place(Place::Session);
    assert!(!rise_to_chunk(&mut w, "nowhere"));
    assert_eq!(w.place(), Some(Place::Session));
    assert_eq!(selected(&w), None);
}

/// The identity chip rises to the chunk the rail names and nothing
/// when the rail names none.
#[test]
fn the_session_chip_rises_to_the_rails_chunk() {
    let mut w = window().with_place(Place::Session);
    assert_eq!(w.rail().chunk(), Some("here"));
    session_chunk_click(&mut w);
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(selected(&w), Some("here"));

    let mut bare = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    bare.place = Place::Session;
    session_chunk_click(&mut bare);
    assert_eq!(bare.place(), Some(Place::Session), "no chunk, no rise");
}

// ── anchors and verbs ───────────────────────────────────────────────────

/// A star is resolved by its graph index at press time: the id the
/// paint carried is never trusted, so a re-seed between the frame and
/// the press opens whatever is at that place now.
#[test]
fn a_star_click_resolves_the_chunk_by_graph_index_at_press_time() {
    let mut w = window().with_place(Place::Sky);
    let ids = w.roadmap().graph_ids();
    let at = ids
        .iter()
        .position(|id| *id == "next")
        .expect("next is on the graph");
    assert_eq!(sky_star_click(&mut w, at), vec![]);
    assert_eq!(selected(&w), Some("next"));
    assert_eq!(sky_star_click(&mut w, 99), vec![], "no star there");
    assert_eq!(selected(&w), Some("next"), "and nothing moved");
}

/// Start and Complete are asks for the engine port on the selected
/// chunk by id; with nothing selected there is nothing to ask.
#[test]
fn start_and_complete_ask_the_engine_for_the_selected_chunk() {
    let mut w = window();
    assert_eq!(roadmap_chunk_click(&w, false), vec![]);
    assert!(open_chunk(&mut w, "next"));
    assert_eq!(
        roadmap_chunk_click(&w, false),
        vec![Cmd::Engine(EngineCmd::StartChunk { id: "next".into() })]
    );
    assert!(
        roadmap_chunk_click(&w, true).is_empty(),
        "pending work cannot complete"
    );
    assert!(open_chunk(&mut w, "here"));
    assert_eq!(
        roadmap_chunk_click(&w, true),
        vec![Cmd::Engine(EngineCmd::CompleteChunk { id: "here".into() })]
    );
}

/// The filter and the sort own the list's window: each cycles or sorts
/// and puts the list back at its top.
#[test]
fn the_filter_and_the_sort_reset_the_lists_window() {
    let mut w = window();
    assert_eq!(w.roadmap().filter_label(), "Active");
    filter_click(&mut w);
    assert_eq!(w.roadmap().filter_label(), "In progress");
    assert_eq!(
        w.roadmap()
            .rows()
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        vec!["here", "stuck"],
        "the in-progress filter keeps the stuck chunk too"
    );
    let column = sort_column(RoadmapSort::Name.widget()).expect("a sort header");
    sort_click(&mut w, column);
    assert_eq!(w.roadmap().sort(), RoadmapSort::Name);
    assert_eq!(sort_column(Widget::RoadmapReset), None);
}

#[test]
fn track_focus_selection_search_and_draft_survive_browsing_on_both_surfaces() {
    use crate::cockpit::{TrackFocus, Widget};
    use crate::input::Modifiers;
    for surface in [Surface::Terminal, Surface::Native] {
        let mut model = seeded(surface, None).with_tab(StageTab::Roadmap);
        row_click(&mut model, 0);
        let selection = model.selected().cloned();
        model.inspector_scroll = 23;
        model.pane = Pane::Stage;
        model.roadmap.focus_track_row(0);
        let commands = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        assert!(commands.is_empty());
        assert_eq!(model.selected(), selection.as_ref());
        assert_eq!(model.inspector_scroll, 23);
        assert!(matches!(
            model.roadmap.track_focus(),
            Some(TrackFocus::Track(_))
        ));
        model.session.prompt.insert("Keep this draft");
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Char('/')).with(Modifiers::ALT)),
        );
        update(&mut model, Msg::Paste("shipped".into()));
        assert_eq!(model.roadmap.track_query(), "shipped");
        assert!(
            model
                .roadmap
                .track_rows()
                .iter()
                .any(|row| row.label() == "shipped")
        );
        assert_eq!(model.prompt().text(), "Keep this draft");
        browse_control(&mut model, Widget::RoadmapSearchClear);
        assert!(!model.roadmap.is_selected_visible());
        browse_control(&mut model, Widget::RoadmapReveal);
        assert!(model.roadmap.is_selected_visible());
        assert_eq!(model.selected(), selection.as_ref());
        assert_eq!(model.inspector_scroll, 23);
        model.show(StageTab::Turn);
        model.show(StageTab::Roadmap);
        assert_eq!(model.selected(), selection.as_ref());
        assert_eq!(model.prompt().text(), "Keep this draft");
        browse_control(&mut model, Widget::RoadmapDependencies);
        assert!(model.roadmap.is_dependencies_view());
        assert_eq!(model.rail.chunk(), Some("here"));
    }
}

#[test]
fn revealing_selected_work_finishes_search_and_focuses_the_selected_row() {
    let mut model = seeded(Surface::Terminal, None).with_tab(StageTab::Roadmap);
    row_click(&mut model, 0);
    browse_control(&mut model, Widget::RoadmapSearch);
    search_query(&mut model, "shipped".into());
    assert!(model.roadmap.is_track_search_editing());
    browse_control(&mut model, Widget::RoadmapReveal);
    assert!(!model.roadmap.is_track_search_editing());
    assert!(model.roadmap.track_query().is_empty());
    assert!(model.roadmap.is_selected_visible());
    assert_eq!(model.pane(), Pane::Stage);
}

#[test]
fn live_tracks_keep_list_anchor_selected_reading_and_search_return_position() {
    use crate::cockpit::{Inspection, TrackFocus};
    use serde_json::json;
    let chunks = crate::harness::fixtures::chunks_from(
        (0..80)
            .map(|n| {
                json!({
                    "id":format!("item-{n:03}"), "name":format!("Work {n:03}"), "status":"pending",
                    "priority":100+n, "group":"Core", "description":"Keep the selected reader.",
                })
            })
            .collect(),
    );
    let mut model = Model::new(Size::new(120, 40))
        .with_tab(StageTab::Roadmap)
        .with_snapshot(&magistr_workflow_contracts::NativeSnapshot {
            chunks,
            ..Default::default()
        });
    row_click(&mut model, 30);
    model.pane = Pane::Stage;
    model.inspector_scroll = 12;
    let card = Inspection::of(&model).unwrap();
    let section = card.sections[0].key.clone();
    model
        .inspection_state
        .borrow_mut()
        .expanded
        .insert(section.clone());
    model.session.prompt.insert("Preserve my draft");
    model.roadmap_request_scroll = 7;
    let before = browse_anchor(&model).unwrap();
    let top = model.roadmap_top();
    search_query(&mut model, "Work 079".into());
    search_query(&mut model, String::new());
    assert_eq!(model.roadmap_top(), top);
    assert_eq!(
        model.roadmap.track_focus(),
        Some(&TrackFocus::Item("item-030".into()))
    );
    crate::harness::Scenes::engine_frame(
        &mut model,
        "roadmap",
        "chunk_added",
        json!({"chunk":{
            "id":"new","name":"New work","status":"pending","priority":1,"group":"Earlier",
        }}),
    );
    assert_eq!(browse_anchor(&model), Some(before));
    assert_eq!(model.selected(), Some(&Selected::Chunk("item-030".into())));
    assert_eq!(model.inspector_scroll, 12);
    assert!(model.inspection_state.borrow().expanded.contains(&section));
    assert_eq!(model.roadmap_request_scroll, 7);
    assert_eq!(model.prompt().text(), "Preserve my draft");
}
