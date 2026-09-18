use magistr_domain::policy::RiskTier;

use super::*;
use crate::cockpit::{Msg, Permission, Size, StageTab, Surface, Widget, update};
use crate::harness::Scenes;
use crate::input::{KeyCode, KeyEvent};
use magistr_application::sessions::{AgentEvent, AskPlan, PermissionChoice, PermissionKind};

#[test]
fn shared_navigation_keeps_the_same_identity_and_destination_on_every_surface() {
    use crate::cockpit::Selected;
    use crate::harness::fixtures;
    use magistr_workflow_contracts::NativeSnapshot;
    for surface in [Surface::Terminal, Surface::Native] {
        for pixels in [false, true] {
            let mut model = Model::new(Size::new(120, 40))
                .with_surface(surface)
                .with_tab(StageTab::Turn)
                .with_snapshot(&NativeSnapshot {
                    chunks: fixtures::chunks_from(vec![
                        serde_json::json!({"id":"work", "status":"in_progress"}),
                        serde_json::json!({"id":"done", "status":"done"}),
                    ]),
                    ..NativeSnapshot::default()
                });
            model.pixels = pixels;
            model.session.prompt.insert("Keep this draft");
            for id in ["work", "done"] {
                assert!(crate::cockpit::work::open_chunk(&mut model, id));
                assert_eq!(model.selected(), Some(&Selected::Chunk(id.into())));
                assert_eq!(model.tab(), StageTab::Roadmap);
                assert_eq!(model.shown_tab(), StageTab::Roadmap);
                model.show(StageTab::Session);
                assert_eq!(model.tab(), StageTab::Session);
                assert_eq!(model.shown_tab(), StageTab::Session);
                model.show(StageTab::Turn);
                assert_eq!(model.tab(), StageTab::Turn);
                assert_eq!(model.shown_tab(), StageTab::Turn);
                assert_eq!(model.selected(), Some(&Selected::Chunk(id.into())));
                assert_eq!(model.prompt().text(), "Keep this draft");
            }
        }
    }
}

#[test]
fn switching_views_keeps_selected_reasoning_instead_of_replacing_it() {
    use crate::cockpit::Selected;
    for surface in [Surface::Terminal, Surface::Native] {
        let mut model = Scenes::sky_working(Size::new(120, 40)).with_surface(surface);
        model.pixels = true;
        model.inspect(Selected::Step(917));
        for tab in [StageTab::Roadmap, StageTab::Session, StageTab::Turn] {
            model.show(tab);
            assert_eq!(model.selected(), Some(&Selected::Step(917)));
            assert_eq!(model.tab(), tab);
        }
    }
}

fn journey(surface: Surface) -> Model {
    let mut model = Scenes::navigation(Size::new(120, 40)).with_surface(surface);
    update(
        &mut model,
        Msg::Mode(crate::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    model
}

fn click(model: &mut Model, target: crate::widget_id::WidgetId) {
    use crate::input::{Button, MouseEvent, MouseKind};
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(target),
        },
    );
}

#[test]
fn navigation_journey_preserves_draft_attachment_and_source_anchor() {
    use crate::cockpit::Selected;
    use crate::cockpit::session_item::SessionItem;
    for surface in [Surface::Terminal, Surface::Native] {
        let mut model = journey(surface);
        let prompt = model.prompt().clone();
        let anchor = model
            .stage_anchor()
            .expect("the fixture is reading earlier text");
        assert!(prompt.text().contains("@src/retry.rs"));
        assert!(model.turn().len() > model.stage_rows());
        let change = model.changes().entries()[0].identity();
        let receipt = SessionItem::collect(model.trace(), model.changes())
            .into_iter()
            .map(|row| row.selection())
            .find(|s| matches!(s, Selected::Receipt(_)))
            .expect("real recorded check");
        for selected in [
            Selected::Chunk("retry-work".into()),
            Selected::Step(917),
            Selected::Change(change),
            receipt,
            Selected::Signal("retry-signal".into()),
        ] {
            open_record(&mut model, selected.clone());
            assert_eq!(model.selected(), Some(&selected));
            for tab in [
                StageTab::Turn,
                StageTab::Roadmap,
                StageTab::Session,
                StageTab::Turn,
            ] {
                model.show(tab);
                assert_eq!(model.tab(), tab);
                assert_eq!(model.shown_tab(), tab);
                assert_eq!(model.selected(), Some(&selected));
                assert_eq!(model.prompt(), &prompt);
                assert_eq!(model.stage_anchor(), Some(anchor));
            }
        }
    }
}

#[test]
fn navigation_journey_resolves_rows_links_and_stale_records_by_identity() {
    use crate::cockpit::{Selected, WorkbenchEvent};
    for surface in [Surface::Terminal, Surface::Native] {
        let mut model = journey(surface);
        click(&mut model, Widget::trace_row(0));
        assert_eq!(model.selected(), Some(&Selected::Step(917)));
        click(&mut model, Widget::LinkedWork.id());
        assert_eq!(
            model.selected(),
            Some(&Selected::Chunk("retry-work".into()))
        );
        assert_eq!(model.tab(), StageTab::Roadmap);
        let identity = model.changes().entries()[0].identity();
        click(&mut model, Widget::change_row(0));
        assert_eq!(model.selected(), Some(&Selected::Change(identity.clone())));
        update(
            &mut model,
            Msg::Workbench(WorkbenchEvent::Journal { entries: vec![] }),
        );
        assert_eq!(model.selected(), Some(&Selected::Change(identity)));
        assert!(crate::cockpit::changes::open_click(&model, 0).is_empty());
        assert!(!crate::cockpit::work::open_chunk(
            &mut model,
            "removed-work"
        ));
        assert!(matches!(model.selected(), Some(Selected::Change(_))));
        click(&mut model, Widget::signal_row(0));
        assert_eq!(
            model.selected(),
            Some(&Selected::Signal("retry-signal".into()))
        );
    }
}

#[test]
fn navigation_journey_refuses_an_unoffered_answer_without_losing_context() {
    use crate::cockpit::{DecisionIntent, Selected};
    for surface in [Surface::Terminal, Surface::Native] {
        let mut model = journey(surface);
        open_record(&mut model, Selected::Step(917));
        let prompt = model.prompt().clone();
        update(
            &mut model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 73,
                title: "Review this change".into(),
                choices: vec![PermissionChoice::new("deny", PermissionKind::RejectOnce)],
                edit: false,
                risk: RiskTier::Medium,
                plan: None,
            }),
        );
        let commands = update(
            &mut model,
            Msg::Decision(DecisionIntent::Answer(PermissionKind::AllowOnce)),
        );
        assert!(
            !commands
                .iter()
                .any(|cmd| matches!(cmd, Cmd::Permission { .. }))
        );
        assert_eq!(model.dock().len(), 1);
        assert_eq!(model.prompt(), &prompt);
        assert_eq!(model.selected(), Some(&Selected::Step(917)));
    }
}

fn window() -> Model {
    Model::new(Size::new(120, 40)).with_surface(Surface::Native)
}

fn press(model: &mut Model, code: KeyCode) -> Vec<Cmd> {
    update(model, Msg::Key(KeyEvent::new(code)))
}

/// Esc through the real fold: whatever the motion layer adds, it is
/// never an answer.
fn esc_press(model: &mut Model) {
    let cmds = press(model, KeyCode::Esc);
    assert!(
        !cmds.iter().any(|cmd| matches!(cmd, Cmd::Permission { .. })),
        "never an answer: {cmds:?}"
    );
}

/// An ask on the dock: a plan-carrying one is the Moment's card, an
/// edit is the review, and a bare one is the strip's line only.
fn ask(id: u64, edit: bool, plan: bool) -> Msg {
    Msg::Agent(AgentEvent::PermissionRequest {
        id,
        title: format!("ask {id}"),
        choices: vec![
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
        ],
        edit,
        risk: RiskTier::Medium,
        plan: plan.then(|| AskPlan::new(["one step"])),
    })
}

/// The fold is the window's: a terminal's Esc is not its to take, and
/// the terminal keeps its tab.
#[test]
fn a_terminal_esc_is_not_the_folds() {
    let mut terminal = Model::new(Size::new(80, 24));
    assert_eq!(esc(&mut terminal), None);
    assert_eq!(terminal.tab(), StageTab::Roadmap);
    assert_eq!(terminal.place(), None);
}

/// The last stop: one altitude up, and the Galaxy holds — Esc is spent
/// either way, so it never falls through to the prompt.
#[test]
fn esc_climbs_one_altitude_and_holds_at_the_galaxy() {
    let mut w = window();
    let mut trail = vec![w.place()];
    for _ in 0..3 {
        assert_eq!(esc(&mut w), Some(Vec::new()));
        trail.push(w.place());
    }
    assert_eq!(
        trail,
        [
            Some(Place::Session),
            Some(Place::Sky),
            Some(Place::Galaxy),
            Some(Place::Galaxy)
        ]
    );
    for place in Place::ALL {
        let mut w = window();
        w.place = place;
        esc(&mut w);
        assert_eq!(w.place(), Some(place.up().unwrap_or(place)), "{place:?}");
    }
}

/// The path at a record (altitude 3) is four stops — `projects ›
/// project › chunk › record` — the record the inert here stop wearing
/// the record crumb, the chunk pressing back to the Session; with no
/// chunk known the project stays a back stop and the record is still
/// here. The Session keeps its three.
#[test]
fn the_path_at_a_record_is_four_stops_with_the_record_here() {
    use crate::engine::EngineEvent;
    let mut w = window();
    update(
        &mut w,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                progress: crate::harness::fixtures::progress(1, 2),
                chunks: vec![crate::harness::fixtures::chunk_from(
                    serde_json::json!({"id": "gui2-2a", "status": "in_progress"}),
                )],
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    for (place, label) in [(Place::Trace, "trace"), (Place::Changes, "changes")] {
        w.place = place;
        let path = w.path();
        let stops: Vec<(Widget, &str, bool)> = path
            .iter()
            .map(|stop| (stop.widget, stop.label.as_str(), stop.here))
            .collect();
        assert_eq!(
            stops,
            [
                (Widget::CrumbProjects, "projects", false),
                (Widget::CrumbProject, "magistr", false),
                (Widget::CrumbSession, "gui2-2a", false),
                (Widget::CrumbRecord, label, true),
            ],
            "{place:?}"
        );
    }
    w.place = Place::Session;
    assert_eq!(w.path().len(), 3, "the Session names the chunk as here");
    assert!(w.path()[2].here && w.path()[2].widget == Widget::CrumbSession);

    let mut bare = window();
    bare.place = Place::Trace;
    let path = bare.path();
    assert_eq!(path.len(), 3, "no chunk: projects › project › trace");
    assert!(!path[1].here, "the project presses back");
    assert_eq!((path[2].widget, path[2].here), (Widget::CrumbRecord, true));
}

/// The Signals inbox is the project's room, not a session's: its path
/// is three stops whether or not the rail names a chunk, the project
/// pressing back to the Sky and `signals` the here stop.
#[test]
fn the_signals_path_hangs_off_the_project_never_the_session() {
    use crate::engine::EngineEvent;
    let mut w = window();
    update(
        &mut w,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                progress: crate::harness::fixtures::progress(1, 2),
                chunks: vec![crate::harness::fixtures::chunk_from(
                    serde_json::json!({"id": "gui2-2a", "status": "in_progress"}),
                )],
                signals: (0..2)
                    .map(|n| {
                        crate::harness::fixtures::signal_from(
                            serde_json::json!({"id": format!("s{n}"), "status": "new"}),
                        )
                    })
                    .collect(),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    w.place = Place::Signals;
    let path = w.path();
    let stops: Vec<(Widget, &str, bool)> = path
        .iter()
        .map(|stop| (stop.widget, stop.label.as_str(), stop.here))
        .collect();
    assert_eq!(
        stops,
        [
            (Widget::CrumbProjects, "projects", false),
            (Widget::CrumbProject, "magistr", false),
            (Widget::CrumbRecord, "signals", true),
        ]
    );
    assert_eq!(
        Widget::CrumbProject.place_target(Place::Signals),
        Some(Place::Sky),
        "the project stop climbs to the Sky"
    );
    assert_eq!(esc(&mut w), Some(Vec::new()));
    assert_eq!(w.place(), Some(Place::Sky), "Esc climbs to the Sky");
}

/// The first stop: the Brief goes away ahead of every other meaning,
/// and the camera does not move for it.
#[test]
fn the_brief_stands_down_first() {
    let mut w = Scenes::briefed(Size::new(120, 40));
    assert!(w.brief().is_some());
    let at = w.place();
    esc_press(&mut w);
    assert!(w.brief().is_none(), "the Brief went first");
    assert_eq!(w.place(), at, "and the camera held");
}

/// The second stop: a viewer pane on the Changes journal or the Trace
/// feed closes before the altitude moves, and only at its own place.
#[test]
fn a_viewer_pane_closes_before_the_altitude_moves() {
    let mut w = window();
    w.place = Place::Changes;
    w.changes
        .open_viewer("src/main.rs", vec!["fn main() {}".to_owned()]);
    esc_press(&mut w);
    assert!(w.changes().viewer().is_none(), "the pane closed");
    assert_eq!(w.place(), Some(Place::Changes), "the camera held");
    esc_press(&mut w);
    assert_eq!(w.place(), Some(Place::Session), "then up");

    let mut w = window();
    w.place = Place::Trace;
    w.trace.open_viewer(vec!["open hypotheses: 1".to_owned()]);
    esc_press(&mut w);
    assert!(w.trace().viewer().is_none(), "the pane closed");
    assert_eq!(w.place(), Some(Place::Trace));

    // A pane open on another altitude's tab is not this place's.
    let mut w = window();
    w.place = Place::Session;
    w.changes.open_viewer("src/main.rs", vec![]);
    esc_press(&mut w);
    assert!(w.changes().viewer().is_some(), "not the Session's pane");
    assert_eq!(w.place(), Some(Place::Sky));
}

/// A card open on the feed folds before the altitude
/// moves — collapse is not navigation — the newest first, one per
/// press; a card open at another altitude is not that altitude's.
#[test]
fn an_open_card_folds_before_the_altitude_moves() {
    use magistr_application::sessions::{ToolStatus, ToolVerb};
    let open = |w: &Model| {
        w.turn()
            .lines()
            .iter()
            .filter(|line| {
                matches!(
                    line.kind(),
                    crate::turn::LineKind::Tool { expanded: true, .. }
                )
            })
            .count()
    };
    let mut w = window();
    for id in ["c1", "c2"] {
        update(
            &mut w,
            Msg::Agent(AgentEvent::ToolCall {
                id: id.to_owned(),
                title: "ls".to_owned(),
                verb: ToolVerb::Run,
                status: ToolStatus::Completed,
                diff: None,
                images: Vec::new(),
                record: Box::default(),
            }),
        );
    }
    assert!(w.turn_mut().toggle_at_row(0));
    assert!(w.turn_mut().toggle_at_row(2));
    assert_eq!(open(&w), 2);
    esc_press(&mut w);
    assert_eq!(open(&w), 1, "one card folded");
    assert_eq!(w.place(), Some(Place::Session), "and the camera held");
    esc_press(&mut w);
    assert_eq!(open(&w), 0);
    assert_eq!(w.place(), Some(Place::Session));
    esc_press(&mut w);
    assert_eq!(w.place(), Some(Place::Sky), "then up");

    // Open at the Sky: not the Sky's to fold.
    let mut w = window();
    update(
        &mut w,
        Msg::Agent(AgentEvent::ToolCall {
            id: "c9".to_owned(),
            title: "ls".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Box::default(),
        }),
    );
    assert!(w.turn_mut().toggle_at_row(0));
    w.place = Place::Sky;
    esc_press(&mut w);
    assert_eq!(open(&w), 1, "the card stays open");
    assert_eq!(w.place(), Some(Place::Galaxy));
}

/// The third stop, the user's law: Esc on a Moment stands
/// the card down to the Needs-you dock unanswered — never inert, never
/// an answer — and the camera returns to the Session. The ask waits
/// on the strip, answers from there, and the answer holds the camera.
#[test]
fn a_moment_stands_down_to_the_dock_and_the_camera_returns_to_the_session() {
    let mut w = window();
    w.place = Place::Sky;
    update(&mut w, ask(7, false, true));
    assert!(w.moment().is_some(), "the card came to the Sky");
    esc_press(&mut w);
    assert!(w.moment().is_none(), "the card stood down");
    assert_eq!(w.permissions_pending(), 1, "the ask still waits");
    assert_eq!(w.dock().len(), 1);
    assert_eq!(w.place(), Some(Place::Session), "the camera returned");
    // The strip is not modal: Esc is navigation again.
    esc_press(&mut w);
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(w.permissions_pending(), 1, "still never an answer");
    // The strip answers, and the answer holds the camera.
    let cmds = press(&mut w, KeyCode::Char('a'));
    assert!(cmds.contains(&Cmd::Permission {
        id: 7,
        option_id: "allow".to_owned()
    }));
    assert_eq!(w.place(), Some(Place::Sky));
    assert_eq!(w.permissions_pending(), 0);
}

/// A condition being written is cleared before the card stands down —
/// the card's own rule for Esc, kept: lose the sentence, not the
/// decision.
#[test]
fn a_written_condition_clears_before_the_card_stands_down() {
    let mut w = window();
    update(&mut w, ask(1, false, true));
    press(&mut w, KeyCode::Char('x'));
    assert!(w.condition_open(), "x opened the register");
    esc_press(&mut w);
    assert!(!w.condition_open(), "the sentence is gone");
    assert!(w.moment().is_some(), "the decision survived the clear");
    esc_press(&mut w);
    assert!(w.moment().is_none(), "then the card stood down");
}

/// The review is the other Moment and stands down the same way.
#[test]
fn a_review_stands_down_the_same_way() {
    let mut w = window();
    update(&mut w, ask(3, true, false));
    assert!(w.review().is_some());
    esc_press(&mut w);
    assert!(w.review().is_none(), "the review stood down");
    assert_eq!(w.permissions_pending(), 1);
    assert_eq!(w.place(), Some(Place::Session));
}

/// Standing down is per ask: the one stood down waits on the strip,
/// and the next ask to reach the front raises its own card.
#[test]
fn the_next_ask_raises_its_own_card() {
    let mut w = window();
    update(&mut w, ask(1, false, true));
    update(&mut w, ask(2, false, true));
    press(&mut w, KeyCode::Esc);
    assert!(w.moment().is_none(), "ask 1 stood down");
    press(&mut w, KeyCode::Char('a'));
    assert_eq!(w.permissions_pending(), 1, "ask 1 answered from the strip");
    assert!(w.moment().is_some(), "ask 2 is the card now");
    assert_eq!(w.moment().map(Permission::id), Some(2));
}

/// The dock lowers ahead of any altitude move: with the running card's screen raised,
/// Esc lowers it and the camera holds; the next Esc folds the open card;
/// only then does the camera climb. Away from the Session the dock is
/// not what Esc peels.
#[test]
fn esc_lowers_the_dock_before_folding_a_card_or_climbing() {
    use crate::cockpit::Place;
    use crate::input::Modifiers;
    let mut w = Scenes::terminal_pane(Size::new(120, 40)).with_surface(Surface::Native);
    Scenes::settle(&mut w);
    update(
        &mut w,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER)),
    );
    assert!(w.term_dock().is_up());
    assert_eq!(w.place(), Some(Place::Session));
    assert_eq!(esc(&mut w), Some(Vec::new()));
    assert!(!w.term_dock().is_up(), "the dock lowered first");
    assert_eq!(w.place(), Some(Place::Session), "and the camera held");
    assert_eq!(esc(&mut w), Some(Vec::new()));
    assert_eq!(w.place(), Some(Place::Session), "then the card folded");
    assert_eq!(esc(&mut w), Some(Vec::new()));
    assert_eq!(w.place(), Some(Place::Sky), "then it climbed");
    // Raised again and the camera moved elsewhere: Esc is the place's.
    w.place = Place::Session;
    update(
        &mut w,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER)),
    );
    w.place = Place::Changes;
    assert_eq!(esc(&mut w), Some(Vec::new()));
    assert!(w.term_dock().is_up(), "the dock is the Session's to lower");
    assert_eq!(
        w.place(),
        Some(Place::Session),
        "Changes climbed as it does"
    );
}

/// The header's session stop names when the session started: a
/// label the entry point supplies, drawn after the here stop at the
/// Session only — a record's path keeps the chunk pressing back with
/// no started stop, and a window told no start time keeps the path it
/// had.
#[test]
fn the_session_stop_names_when_the_session_started() {
    use crate::engine::EngineEvent;
    assert_eq!(window().session_started(), None);
    let mut w = window().with_session_started("09:02");
    assert_eq!(w.session_started(), Some("09:02"));
    w.place = Place::Session;
    let path = w.path();
    let stops: Vec<(Widget, &str, bool)> = path
        .iter()
        .map(|stop| (stop.widget, stop.label.as_str(), stop.here))
        .collect();
    assert_eq!(
        stops,
        [
            (Widget::CrumbProjects, "projects", false),
            (Widget::CrumbProject, "magistr", true),
            (Widget::CrumbSession, "session \u{b7} 09:02", false),
        ],
        "no chunk: the project is here, the started stop after it"
    );
    update(
        &mut w,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                chunks: vec![crate::harness::fixtures::chunk_from(
                    serde_json::json!({"id": "gui2-2a", "status": "in_progress"}),
                )],
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    let path = w.path();
    let stops: Vec<(Widget, &str, bool)> = path
        .iter()
        .map(|stop| (stop.widget, stop.label.as_str(), stop.here))
        .collect();
    assert_eq!(
        stops,
        [
            (Widget::CrumbProjects, "projects", false),
            (Widget::CrumbProject, "magistr", false),
            (Widget::CrumbSession, "gui2-2a", true),
            (Widget::CrumbSession, "session \u{b7} 09:02", false),
        ],
        "on a chunk: the chunk is here, the started stop after it"
    );
    w.place = Place::Trace;
    let path = w.path();
    assert_eq!(
        path.len(),
        4,
        "a record: projects › project › chunk › trace"
    );
    assert!(
        path.iter().all(|stop| !stop.label.starts_with("session")),
        "no started stop at a record"
    );
}
