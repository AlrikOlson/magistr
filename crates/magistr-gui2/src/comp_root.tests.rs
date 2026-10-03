use std::time::Duration;

use magistr_cockpit::cockpit::{
    FrameClock, LoopEvent, LoopStage, Model, Msg, Size, Widget, update,
};

use super::*;
use crate::comp_feed::FeedComposition;
use crate::comp_moment::MomentComposition;
use crate::element::Kind;
use crate::fixture::Fixture;

const CELL: (f32, f32) = (9.0, 20.0);
const VIEWPORT: (f32, f32) = (1100.0, 640.0);

fn clock() -> FrameClock {
    FrameClock::new(0, Duration::ZERO)
}

fn root_of(view: &str) -> (Model, Element) {
    let mut model = Fixture::mock(view, Size::new(120, 40)).expect("the view has a fixture");
    // The frame's own declaration: every frame this crate draws is on
    // a window (`Frame::build` sets it before mapping the root).
    model.set_surface(magistr_cockpit::cockpit::Surface::Native);
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    (model, root)
}

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

fn has(root: &Element, needle: &str) -> bool {
    texts(root).iter().any(|t| t.contains(needle))
}

fn has_id(root: &Element, id: ElementId) -> bool {
    let mut found = false;
    root.each(&mut |_, element| found |= element.id == Some(id));
    found
}

#[test]
fn session_workspace_keeps_conversation_reasoning_work_and_details_together() {
    let (model, root) = root_of("05");
    assert!(!model.reasoning_expanded());
    for heading in ["Conversation", "Reasoning", "Nearby work", "Details"] {
        assert!(has(&root, heading), "missing {heading}");
    }
    assert!(has_id(&root, ComposerComposition::PILL));
    assert!(!model.uses_flight_recorder());
}

/// The session's cap is the identity band: the chunk chip, the agent's identity, the
/// iteration pair and the stage word — which says where the loop stands between agents
/// (the gates, the judge) instead of the feed's hangup being read as a fault
/// (loop-gap-not-a-fault), and `paused` / `loop ended` when it stopped moving. The chip
/// routes to the cockpit's own widget.
#[test]
fn the_session_cap_is_the_identity_band_and_names_the_gap_between_agents() {
    let (mut model, root) = root_of("05");
    assert!(has(&root, "gui2-2a \u{b7} The design language"));
    assert!(has(&root, "sonnet \u{b7} Draft mode"));
    assert!(has(&root, "4 / 12"));
    assert!(has(&root, "agent working"), "the agent's own stage word");
    assert!(!has(&root, "Iteration"), "no template word on the band");
    assert_eq!(
        RootComposition::target_of(&model, BandComposition::CHUNK),
        Some(Widget::SessionChunk.id()),
        "the chunk chip is the cockpit's own control"
    );
    update(&mut model, Msg::Loop(LoopEvent::Stage(LoopStage::Gates)));
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    assert!(has(&root, "gates running"));
    assert!(!has(&root, "agent working"));
    update(&mut model, Msg::Loop(LoopEvent::Stage(LoopStage::Judge)));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("gates failed".to_owned())),
    );
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    assert!(has(&root, "paused"));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("max iterations".to_owned())),
    );
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    assert!(has(&root, "loop ended"));
}

/// Roadmap keeps its graph/list and item views beside a resizable boundary.
#[test]
fn roadmap_keeps_its_own_resizable_columns() {
    for view in ["03", "04"] {
        let (model, root) = root_of(view);
        assert_eq!(model.place(), Some(Place::Sky), "view {view} is the Sky");
        assert!(
            has_id(
                &root,
                crate::comp_workspace::WorkspaceComposition::id(Widget::PanelColumns.id())
            ),
            "roadmap columns on view {view}"
        );
        assert!(!has(&root, "Checkpoints"), "no ledger on view {view}");
    }
    let (_, root) = root_of("05");
    assert!(
        has_id(
            &root,
            crate::comp_workspace::WorkspaceComposition::id(Widget::PanelColumns.id())
        ),
        "the session keeps its rail"
    );
}

/// While a plain ask fronts the dock the session cap names the wait in the strip's own
/// words — a parked loop is never silent — and the ask card floats over the column,
/// whose cap and composer stay, unlike behind the plan Moment.
#[test]
fn the_session_cap_names_the_wait_while_a_plain_ask_fronts_the_dock() {
    use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind};
    use magistr_domain::policy::RiskTier;
    let (mut model, _) = root_of("05");
    update(
        &mut model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 2,
            title: "roadmap_start_chunk".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    assert!(
        has(&root, "needs you: Start a roadmap item"),
        "the band names the wait"
    );
    assert!(has(&root, "4 / 12"), "beside the iteration");
    assert!(has(&root, "needs you"), "the ask card is up");
    assert!(
        has(&root, "Message Magistr"),
        "the composer stays under the card"
    );
    assert_eq!(
        RootComposition::target_of(&model, ElementId(MomentComposition::ANSWERS)),
        Some(Widget::Allow.id()),
        "the card's Allow is the strip's own answer"
    );
    update(&mut model, click_target(Widget::Allow.id()));
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    assert!(
        !has(&root, "needs you"),
        "answered, the wait and the card are gone"
    );
}

/// The ledger head's verbs resolve to the cockpit's loop widgets, and a press on Step
/// reaches the driver through the one fold the keys take.
#[test]
fn the_ledger_heads_verbs_resolve_to_the_loop_widgets_and_click_through() {
    use magistr_cockpit::cockpit::{Cmd, LoopCmd};
    let (mut model, _) = root_of("05");
    let root = RailComposition::el(&model, Medium::Night, CELL.0);
    for id in [
        LedgerVerbs::PAUSE,
        LedgerVerbs::STEP,
        LedgerVerbs::CHECKPOINT,
        LedgerVerbs::ABORT,
    ] {
        assert!(has_id(&root, id), "{id:?} is on the rail");
    }
    let target = |id| RootComposition::target_of(&model, id);
    assert_eq!(target(LedgerVerbs::PAUSE), Some(Widget::LoopPause.id()));
    assert_eq!(target(LedgerVerbs::STEP), Some(Widget::LoopStep.id()));
    assert_eq!(
        target(LedgerVerbs::CHECKPOINT),
        Some(Widget::LoopCheckpoint.id())
    );
    assert_eq!(target(LedgerVerbs::ABORT), Some(Widget::LoopAbort.id()));
    let cmds = update(&mut model, click_target(Widget::LoopStep.id()));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Step)), "{cmds:?}");
    let cmds = update(&mut model, click_target(Widget::LoopAbort.id()));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Abort)), "{cmds:?}");
}

/// A press the window resolved to `id`.
fn click_target(id: WidgetId) -> Msg {
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
        target: Some(id),
    }
}

/// The cap's live dot: nothing at the foot of the wave — the settled still is the
/// volume's — and a lit disc with its halo at the crest, pinned in the margin so the
/// cap never moves.
#[test]
fn the_session_cap_wears_a_live_dot_that_breathes_off_the_foot() {
    let (mut model, root) = root_of("05");
    assert!(model.is_working(), "view 05's loop is running");
    let discs = |root: &Element| {
        let mut discs = Vec::new();
        root.each(&mut |_, element| {
            if element.pin[0].is_some() && element.glow.is_some() {
                discs.push(element.glow);
            }
        });
        discs
    };
    assert!(discs(&root).is_empty(), "the foot: no light");
    for _ in 0..(magistr_cockpit::cockpit::Motion::GLOW_STEPS / 2) {
        update(&mut model, Msg::Tick);
    }
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    assert_eq!(discs(&root).len(), 1, "the crest: the dot's own light");
    assert!(has(&root, "4 / 12"), "the band did not move");
}

/// View 05 — the whole session in one tree: the altitude path, the
/// session cap, the composer, the rail with its divider, and the foot.
#[test]
fn the_session_root_arranges_header_turn_rail_and_foot() {
    let (_, root) = root_of("05");
    assert!(has(&root, "projects"), "the altitude path");
    assert!(has(&root, "4 / 12"), "the session cap: the identity band");
    assert!(has(&root, "Message Magistr"), "the composer at rest");
    assert!(has(&root, "Nearby work"), "nearby work rides right");
    assert!(has(&root, "38,204 tok"), "the foot's total");
    assert!(
        has_id(
            &root,
            crate::comp_workspace::WorkspaceComposition::id(Widget::PanelColumns.id())
        ),
        "the drag handle"
    );
    assert!(
        has_id(&root, RootComposition::STAGE),
        "the feed's viewport id"
    );
}

/// Becoming idle never removes Roadmap navigation or the request composer.
#[test]
fn idle_roadmap_keeps_navigation_and_request_controls() {
    use magistr_cockpit::cockpit::{Motion, Msg, update};
    let mut model = Fixture::mock("03", Size::new(120, 40)).expect("view 03 has a fixture");
    model.set_surface(magistr_cockpit::cockpit::Surface::Native);
    for _ in 0..(Motion::REST_STEPS + Motion::CHROME_FADE_STEPS) {
        update(&mut model, Msg::Tick);
    }
    assert!(model.sky_at_rest(), "view 03 is at rest");
    let root = RootComposition::el(&model, Medium::Night, CELL, VIEWPORT, &clock());
    for label in [
        "projects",
        "Work items",
        "Requests",
        "Request roadmap changes",
    ] {
        assert!(has(&root, label), "idle Roadmap retains {label}");
    }
}

/// View 06 — a Moment: the card floats on a scrim, the rail stands
/// down, and the decision's phase is the only word on the foot.
#[test]
fn a_moment_takes_the_screen_and_the_rail_stands_down() {
    let (model, root) = root_of("06");
    assert!(model.moment().is_some(), "view 06 is a Moment");
    assert!(!has(&root, "Checkpoints"), "no rail behind a Moment");
    assert!(has(&root, "plan \u{b7} two-phase"), "the plan card's cap");
    assert!(!has(&root, "tok"), "no totals on a decision screen");
}

/// View 07 — a review replaces the session: the change is the thing to
/// look at, under the same altitude path.
#[test]
fn a_review_replaces_the_session_under_the_same_path() {
    let (model, root) = root_of("07");
    assert!(model.review().is_some(), "view 07 is a review");
    assert!(has(&root, "projects"), "the path stays");
    assert!(has(&root, "Review"), "the review surface");
    assert!(!has(&root, "Message Magistr"), "the composer stands down");
}

/// The element ids the compositions mint resolve to the cockpit's own
/// widgets — the seam that keeps the one Msg/update loop.
#[test]
fn element_ids_resolve_to_the_cockpits_widgets() {
    let (model, _) = root_of("05");
    let target = |id| RootComposition::target_of(&model, id);
    assert_eq!(
        target(RootComposition::DIVIDER),
        Some(Widget::RailDivider.id())
    );
    assert_eq!(target(RootComposition::STAGE), Some(Widget::Stage.id()));
    assert_eq!(
        target(crate::comp_composer::ComposerComposition::PILL),
        Some(Widget::Composer.id())
    );
    assert_eq!(
        target(crate::comp_term_dock::TermDockComposition::GRIP),
        Some(Widget::TermDockGrip.id())
    );
    assert_eq!(
        target(crate::comp_term_dock::TermDockComposition::PANE),
        Some(Widget::TermDock.id())
    );
    assert_eq!(
        target(ElementId(MomentComposition::ANSWERS)),
        Some(Widget::Allow.id()),
        "answer slot 0 is AllowOnce"
    );
    let top = model.top_row() as u64;
    assert_eq!(
        target(ElementId(FeedComposition::BUTTONS + 2 * top)),
        Some(Widget::edit_open(0)),
        "a feed button at the window's top row is visible index 0"
    );
    assert_eq!(
        target(ElementId(FeedComposition::HEADS + top + 2)),
        Some(Widget::stage_row(2)),
        "a card head is the cockpit's own stage row, by visible index"
    );
    assert_eq!(
        target(ElementId(FeedComposition::HEADS + top + 10_000)),
        None,
        "a head past the window is no target"
    );
    assert_eq!(target(ElementId(99)), None, "an unmapped id is no target");
}

/// At the Galaxy (view 01) the root lays altitude 0: the header wears
/// the running / action-required / ⌘K chips instead of the clock, the
/// field is reserved full-bleed with no supervision rail beside it, and
/// the foot is the Galaxy's own — the ⌘K palette floating over it all.
#[test]
fn the_galaxy_is_the_chips_the_field_and_its_own_foot() {
    use crate::comp_galaxy::GalaxyComposition;
    use crate::comp_palette::PaletteComposition;
    let (model, root) = root_of("01");
    assert_eq!(model.place(), Some(magistr_cockpit::cockpit::Place::Galaxy));
    let texts = texts(&root);
    assert!(texts.iter().any(|t| t == "2 running"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "1 action required"));
    assert!(texts.iter().any(|t| t == "3 projects"), "the Galaxy's foot");
    assert!(
        !texts.iter().any(|t| t == "00:00"),
        "no session clock at the top of the world"
    );
    let mut ids = Vec::new();
    root.each(&mut |_, element| {
        if let Some(id) = element.id {
            ids.push(id);
        }
    });
    assert!(
        ids.contains(&GalaxyComposition::FIELD),
        "the field is reserved"
    );
    assert!(ids.contains(&GalaxyComposition::NEW));
    assert!(
        !ids.contains(&RootComposition::DIVIDER),
        "no rail at altitude 0"
    );
    assert!(
        !ids.contains(&RootComposition::STAGE),
        "no turn column either"
    );
    assert!(
        ids.contains(&ElementId(PaletteComposition::ROWS)),
        "the palette floats over the Galaxy"
    );
}

/// The rail's own controls resolve to the cockpit's widgets through the one seam, and a
/// press on a row's Restore reaches the port as the command `/restore` sends.
#[test]
fn the_rails_rows_and_raise_resolve_to_their_widgets_and_click_through() {
    use crate::budget_meter::BudgetMeter;
    use crate::spine_ledger::SpineLedger;
    use magistr_cockpit::cockpit::{CheckpointCmd, Cmd};
    use magistr_cockpit::input::{MouseEvent, MouseKind};
    let (mut model, _) = root_of("05");
    let root = RailComposition::el(&model, Medium::Night, CELL.0);
    let newest = model.rail().spine().len() - 1;
    let row = ElementId(SpineLedger::ROWS + newest as u64);
    assert!(has_id(&root, row), "the newest row is a control");
    assert!(has_id(&root, BudgetMeter::RAISE), "the meter's Raise");
    assert!(
        has_id(&root, ElementId(RailComposition::CHARTER)),
        "the first charter row"
    );
    let target = |id| RootComposition::target_of(&model, id);
    assert_eq!(target(row), Some(Widget::spine_row(newest)));
    assert_eq!(
        target(ElementId(SpineLedger::RESTORE + 1)),
        Some(Widget::spine_restore(1))
    );
    assert_eq!(
        target(ElementId(SpineLedger::FORK + 1)),
        Some(Widget::spine_fork(1))
    );
    assert_eq!(target(BudgetMeter::RAISE), Some(Widget::BudgetRaise.id()));
    assert_eq!(
        target(ElementId(RailComposition::CHARTER + 2)),
        Some(Widget::charter_row(2))
    );
    assert_eq!(
        target(ElementId(RailComposition::OFFER)),
        Some(Widget::charter_offer(0))
    );
    let cmds = update(&mut model, click_target(Widget::spine_restore(newest)));
    assert_eq!(
        cmds,
        vec![Cmd::Checkpoint(CheckpointCmd::Restore {
            id: "cp-4".to_owned()
        })]
    );
    // The pointer on a row reveals its verbs on the composed root too.
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, 100, 10),
            target: Some(Widget::spine_row(newest)),
        },
    );
    let root = RailComposition::el(&model, Medium::Night, CELL.0);
    assert!(has_id(&root, ElementId(SpineLedger::FORK + newest as u64)));
}

/// The session workspace — the view the desktop opens on — draws the starts beside
/// Roadmap and Commands while no loop is live and the loop's verbs while one is; each
/// resolves to the cockpit's own widget, and a press on a start is the host's command.
#[test]
fn the_conversation_heading_starts_work_while_idle_and_steers_the_loop_while_live() {
    use crate::comp_workspace::WorkspaceComposition;
    use magistr_cockpit::cockpit::{Cmd, LoopCmd};
    let mut idle = Fixture::rail_idle(Size::new(120, 40));
    let root = RootComposition::el(&idle, Medium::Night, CELL, VIEWPORT, &clock());
    for (widget, label) in [
        (Widget::LaunchNext, "Start next item"),
        (Widget::LaunchLoop, "Start loop"),
    ] {
        let id = WorkspaceComposition::id(widget.id());
        assert!(has_id(&root, id) && has(&root, label), "{label}");
        assert_eq!(RootComposition::target_of(&idle, id), Some(widget.id()));
    }
    assert!(!has_id(
        &root,
        WorkspaceComposition::id(Widget::LoopAbort.id())
    ));
    assert!(update(&mut idle, click_target(Widget::LaunchLoop.id())).is_empty());
    assert_eq!(idle.palette().unwrap().title(), "Choose a track");
    assert_eq!(
        RootComposition::target_of(&idle, crate::work_controls::WorkControls::NEXT),
        Some(Widget::LaunchNext.id())
    );
    assert_eq!(
        RootComposition::target_of(&idle, crate::work_controls::WorkControls::LOOP),
        Some(Widget::LaunchLoop.id())
    );

    let (live, _) = root_of("05");
    let mut live = live.with_hosted_work();
    let root = RootComposition::el(&live, Medium::Night, CELL, VIEWPORT, &clock());
    for widget in Widget::LOOP_VERBS {
        assert!(
            has_id(&root, WorkspaceComposition::id(widget.id())),
            "{widget:?}"
        );
    }
    assert!(!has_id(
        &root,
        WorkspaceComposition::id(Widget::LaunchNext.id())
    ));
    let cmds = update(&mut live, click_target(Widget::LoopPause.id()));
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Pause)), "{cmds:?}");
}
