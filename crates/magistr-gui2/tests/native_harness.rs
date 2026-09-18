//! The native pane driven the way a session drives it: producers post
//! on the [`Inbox`], the loop's own [`Route`] folds each message and
//! sends what the core answers back on the [`Commands`] the session
//! takes, and every frame is built by the real painter.
//!
//! This is the cockpit's session-harness shape on the native frame.
//! What it deliberately does **not** do is capture bytes: the terminal
//! harnesses assert an exact wire, and a GPU frame has none — so the
//! assertions here are made of what the frame *says*
//! ([`Frame::says`](magistr_gui2::Frame::says)), which is what the
//! emitter actually drew.

use magistr_application::sessions::{
    AgentEvent, PermissionChoice, PermissionKind, ToolStatus, ToolVerb,
};
use magistr_cockpit::cockpit::{Cmd, FrameClock, Model, Msg, Place, Size, StageTab, Widget};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use magistr_domain::policy::RiskTier;
use magistr_gui2::{AccessTree, ElementId, Frame, MomentComposition};
use tokio::sync::mpsc::UnboundedReceiver;

/// The grid the harness runs on — the measurement size, not the still
/// size: this asks what the pane *does*, not what it looks like.
const SIZE: Size = Size::new(120, 40);

/// A pane: the cockpit's model, the loop's route, and the two seams a
/// producer sees.
struct Pane {
    model: Model,
    frame: Frame,
    route: magistr_gui2::route::Route,
    inbox: Inbox,
    messages: UnboundedReceiver<Msg>,
    commands: Commands,
    quit: bool,
}

impl Pane {
    fn new() -> Self {
        let (inbox, messages) = Inbox::channel();
        let (commands_tx, commands) = Commands::channel();
        Self {
            model: Model::new(SIZE).with_tab(StageTab::Turn),
            frame: Frame::new().expect("the embedded face yields a grid"),
            route: magistr_gui2::route::Route::new(commands_tx),
            inbox,
            messages,
            commands,
            quit: false,
        }
    }

    /// The inbox a producer posts on.
    fn inbox(&self) -> Inbox {
        self.inbox.clone()
    }

    /// Drain everything a producer has posted and fold it, exactly as
    /// the window's forwarder thread and `user_event` do.
    fn drain(&mut self) {
        while let Ok(msg) = self.messages.try_recv() {
            self.quit |= self.route.fold(&mut self.model, msg);
        }
    }

    /// One message straight in — a key or a click, which reach the loop
    /// from winit rather than from the inbox.
    fn fold(&mut self, msg: Msg) {
        self.quit |= self.route.fold(&mut self.model, msg);
    }

    /// Paint a frame and answer what its text runs say, row by row.
    fn paint(&mut self) -> Vec<String> {
        let cell = self.frame.metrics();
        let w = (f32::from(SIZE.cols) * cell.width()) as u32;
        let h = (f32::from(SIZE.rows) * cell.height()) as u32;
        let clock = FrameClock::new(self.model.frames(), std::time::Duration::ZERO);
        self.frame
            .still(&mut self.model, &clock, w, h)
            .expect("the frame renders");
        self.frame
            .says()
            .into_iter()
            .map(|(text, _)| text.trim().to_owned())
            .collect()
    }

    /// Every command the session would have received since the last ask.
    fn commands(&mut self) -> Vec<Cmd> {
        let mut out = Vec::new();
        while let Some(cmd) = self.commands.try_next() {
            out.push(cmd);
        }
        out
    }
}

/// Whether any run on the frame contains `needle`.
fn shows(rows: &[String], needle: &str) -> bool {
    rows.iter().any(|row| row.contains(needle))
}

/// A turn streamed onto the inbox reaches the painted frame — the whole
/// of "a producer drives the native pane through the existing Inbox
/// seam", with no producer knowing which renderer it is feeding.
#[test]
fn a_turn_streamed_on_the_inbox_is_on_the_painted_frame() {
    let mut pane = Pane::new();
    let inbox = pane.inbox();
    // A relay's own sequence: a thought, the user's prompt echo, the
    // answer in two deltas, then the turn's end.
    inbox.send(Msg::Agent(AgentEvent::Thought(
        "reading the roadmap chunk".to_owned(),
    )));
    inbox.send(Msg::Agent(AgentEvent::UserText(
        "paint the pane natively".to_owned(),
    )));
    inbox.send(Msg::Agent(AgentEvent::Text("the pane is ".to_owned())));
    inbox.send(Msg::Agent(AgentEvent::Text("painted".to_owned())));
    pane.drain();
    let rows = pane.paint();
    assert!(shows(&rows, "reading the roadmap chunk"), "{rows:?}");
    assert!(shows(&rows, "paint the pane natively"), "{rows:?}");
    assert!(shows(&rows, "the pane is painted"), "{rows:?}");
    assert!(!pane.quit);
}

/// A tool call and its update land as one card that changes in place,
/// rather than as two — the fold is the cockpit's own, so the pane
/// inherits it.
#[test]
fn a_tool_call_updates_its_own_card_on_the_native_frame() {
    let mut pane = Pane::new();
    let inbox = pane.inbox();
    inbox.send(Msg::Agent(AgentEvent::ToolCall {
        id: "c0".to_owned(),
        title: "read crates/magistr-gui2/src/access.rs".to_owned(),
        verb: ToolVerb::Read,
        status: ToolStatus::Pending,
        diff: None,
        images: Vec::new(),
        record: Default::default(),
    }));
    pane.drain();
    let running = pane.paint();
    assert!(shows(&running, "access.rs"), "{running:?}");
    inbox.send(Msg::Agent(AgentEvent::ToolCallUpdate {
        id: "c0".to_owned(),
        title: None,
        status: Some(ToolStatus::Completed),
        diff: None,
        duration_ms: Some(12),
        images: Vec::new(),
        record: Default::default(),
    }));
    pane.drain();
    let done = pane.paint();
    assert_eq!(
        done.iter().filter(|row| row.contains("access.rs")).count(),
        1,
        "the card changed in place: {done:?}"
    );
}

/// A prompt typed into the pane reaches the commands the session takes —
/// the Commands seam, in the direction the Inbox does not go.
#[test]
fn a_prompt_typed_into_the_pane_reaches_the_session() {
    let mut pane = Pane::new();
    for ch in "ship it".chars() {
        pane.fold(Msg::Key(KeyEvent::new(KeyCode::Char(ch))));
    }
    assert!(pane.commands().is_empty(), "typing sends nothing");
    pane.fold(Msg::Key(KeyEvent::new(KeyCode::Enter)));
    let sent = pane.commands();
    assert!(
        sent.iter()
            .any(|cmd| matches!(cmd, Cmd::Prompt(text) if text == "ship it")),
        "{sent:?}"
    );
    let rows = pane.paint();
    assert!(shows(&rows, "ship it"), "the echo is on screen: {rows:?}");
}

/// A permission request raised by a producer is a card on the window. The dock strip is
/// terminal chrome the window never lays out — asserted, so the strip creeping back is
/// caught here — and a plain ask, the one a live session actually raises (no plan on
/// it), floats as the ask Moment with the stage's cap naming the wait. It is answered
/// by a click on the card's verb or by the keys `dock_key` holds, and both reach the
/// session as the same `Cmd::Permission`.
#[test]
fn a_permission_raised_on_the_inbox_is_a_card_answered_by_click_and_by_key() {
    let ask = |id| {
        Msg::Agent(AgentEvent::PermissionRequest {
            id,
            title: "write src/lib.rs".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("deny", PermissionKind::RejectOnce),
            ],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        })
    };
    let mut pane = Pane::new();
    pane.inbox().send(ask(1));
    pane.drain();
    let rows = pane.paint();
    assert!(
        shows(&rows, "write src/lib.rs"),
        "the ask is on the window as a card: {rows:?}"
    );
    assert!(
        shows(&rows, "needs you: write src/lib.rs"),
        "the stage's cap names the wait: {rows:?}"
    );
    assert!(
        pane.frame.access(1.0).find(Widget::Dock).is_none(),
        "the dock strip is still not placed"
    );
    // A click on the card's Allow verb, resolved by the registry the
    // way the window resolves a press.
    let [x, y, w, h] = pane
        .frame
        .rect_of(ElementId(MomentComposition::ANSWERS))
        .expect("the Allow verb is placed");
    let (px, py) = (f64::from(x + w / 2.0), f64::from(y + h / 2.0));
    let target = pane.frame.hits().hit_at(px, py);
    assert_eq!(target, Some(Widget::Allow.id()));
    let (col, row) = pane.frame.metrics().cell_at(px, py, SIZE);
    pane.fold(Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), col, row).at_px(px as u32, py as u32),
        target,
    });
    let sent = pane.commands();
    assert!(
        sent.iter().any(|cmd| matches!(
            cmd,
            Cmd::Permission { id: 1, option_id } if option_id == "allow"
        )),
        "{sent:?}"
    );
    let rows = pane.paint();
    assert!(
        !shows(&rows, "needs you"),
        "answered, the card and the wait are gone: {rows:?}"
    );
    // The keys still answer — the design's letters are `dock_key`'s, whatever the
    // surface, because the card is model state.
    pane.inbox().send(ask(2));
    pane.drain();
    pane.fold(Msg::Key(KeyEvent::new(KeyCode::Char('d'))));
    let sent = pane.commands();
    assert!(
        sent.iter().any(|cmd| matches!(
            cmd,
            Cmd::Permission { id: 2, option_id } if option_id == "deny"
        )),
        "{sent:?}"
    );
}

/// Primary views and auxiliary destinations remain reachable through shared
/// navigation, while the native frame paints its own composition and access tree.
#[test]
fn every_tab_paints_from_the_one_model() {
    fn open(pane: &mut Pane, query: &str) {
        pane.fold(Msg::Key(
            KeyEvent::new(KeyCode::Char('k')).with(magistr_cockpit::input::Modifiers::SUPER),
        ));
        assert!(pane.model.palette().is_some(), "{query}: palette opened");
        for c in query.chars() {
            pane.fold(Msg::Key(KeyEvent::new(KeyCode::Char(c))));
        }
        assert!(
            !magistr_cockpit::cockpit::Palette::rows(&pane.model).is_empty(),
            "{query}: a destination is offered"
        );
        pane.fold(Msg::Key(KeyEvent::new(KeyCode::Enter)));
        assert!(pane.model.palette().is_none());
    }
    let mut pane = Pane::new();
    pane.model = magistr_cockpit::harness::Scenes::navigation(SIZE);
    // A window draws before any key reaches it, and the frame is what
    // declares the model native (`Frame::build`).
    let _ = pane.paint();
    assert_eq!(
        pane.model.tab(),
        StageTab::Turn,
        "the primary view is shared"
    );
    open(&mut pane, "Search recorded work");
    assert!(pane.model.search().is_some());
    for c in "Saved results".chars() {
        pane.fold(Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    pane.fold(Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert_eq!(
        pane.model
            .roadmap()
            .selected()
            .map(|chunk| chunk.id.as_str()),
        Some("saved-work")
    );
    for (query, place) in [
        ("changes", Place::Changes),
        ("trace", Place::Trace),
        ("Report", Place::Report),
        ("projects", Place::Galaxy),
        ("Roadmap", Place::Sky),
        ("signals", Place::Signals),
        ("", Place::Birth),
        ("Conversation", Place::Session),
    ] {
        if place == Place::Birth {
            pane.fold(Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::GalaxyNew.id()),
            });
        } else {
            if place == Place::Report {
                assert_eq!(
                    pane.model
                        .roadmap()
                        .selected()
                        .map(|chunk| chunk.id.as_str()),
                    Some("saved-work")
                );
            }
            open(&mut pane, query);
        }
        assert_eq!(pane.model.place(), Some(place));
        assert_eq!(pane.model.tab(), place.tab());
        let rows = pane.paint();
        assert!(
            !rows.is_empty(),
            "{place:?}: the frame paints from the one model"
        );
        assert!(
            pane.frame
                .access(1.0)
                .find(Widget::tab(place.tab()))
                .is_none(),
            "{place:?}: the window lays out no tab bar"
        );
        if matches!(place, Place::Galaxy | Place::Sky) {
            assert!(
                !shows(&rows, "next iteration"),
                "{place:?} is the Sky's surface, never the launcher: {rows:?}"
            );
        }
        if place == Place::Session {
            assert!(shows(&rows, "Reviewing delivery"), "{rows:?}");
        }
        // Publish an accessibility tree for every surface. Project navigation, inbox
        // counts, and supervision controls must reflect the selected view.
        let floor = if matches!(
            place,
            Place::Galaxy | Place::Sky | Place::Signals | Place::Birth
        ) {
            3
        } else {
            4
        };
        let tree = pane.frame.access(1.0);
        assert!(
            tree.nodes().len() >= floor,
            "{place:?}: {} nodes",
            tree.nodes().len()
        );
    }
}

/// The accessibility tree the window publishes describes the frame that
/// was just painted: every node it names is a widget the layout placed,
/// with the bounds the painter drew it at.
#[test]
fn the_published_tree_describes_the_frame_that_was_painted() {
    let mut pane = Pane::new();
    pane.inbox()
        .send(Msg::Agent(AgentEvent::Text("streaming".to_owned())));
    pane.drain();
    let _ = pane.paint();
    let tree = pane.frame.access(1.0);
    for node in tree.nodes().iter().skip(1) {
        let (x0, y0, x1, y1) = node.bounds.expect("a placed node has bounds");
        assert!(
            x1 > x0 && y1 > y0,
            "{node:?} names a box the layout did not place"
        );
    }
    let update = tree.update(Some(Widget::Composer));
    assert_eq!(update.nodes.len(), tree.nodes().len());
    assert_eq!(update.focus, AccessTree::node_id(Widget::Composer.id()));
}

/// The rail stands on the right of the window (the design's Session
/// view) and its divider is dragged in pixels: the pointer's pixel goes
/// through the frame's own hit registry to the divider, and the rail
/// lands at exactly the column that pixel names — measured from the
/// window's right edge, since that is the side the rail is on.
#[test]
fn dragging_the_divider_on_the_window_resizes_the_rail_from_the_right() {
    let mut pane = Pane::new();
    pane.inbox()
        .send(Msg::Agent(AgentEvent::Text("a turn to wrap".to_owned())));
    pane.drain();
    let _ = pane.paint();
    let cell = pane.frame.metrics();
    let [stage_x, _, _, _] = pane
        .frame
        .rect_of(magistr_gui2::RootComposition::STAGE)
        .expect("the stage is placed");
    let [dx, dy, dw, dh] = pane
        .frame
        .rect_of(magistr_gui2::comp_workspace::WorkspaceComposition::id(
            Widget::PanelColumns.id(),
        ))
        .expect("the divider is placed");
    assert!(
        dx > stage_x,
        "the divider (and the rail) sit right of the stage"
    );
    let was = pane.model.panel_fraction(
        magistr_cockpit::cockpit::flight_divider::FlightDivider::Columns,
        0.65,
    );

    // Press on the divider's own pixels, resolved by the registry.
    let x = f64::from(dx + dw / 2.0);
    let y = f64::from(dy + dh / 2.0);
    let target = pane.frame.hits().hit_at(x, y);
    assert_eq!(target, Some(Widget::PanelColumns.id()));
    let (col, row) = cell.cell_at(x, y, SIZE);
    pane.fold(Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), col, row).at_px(x as u32, y as u32),
        target,
    });
    assert!(pane.model.active_panel().is_some());

    // Drag eight columns and a third of a cell leftwards, in pixels.
    let to_x = x - f64::from(cell.width()) * 8.0 - f64::from(cell.width()) / 3.0;
    let (col, row) = cell.cell_at(to_x, y, SIZE);
    pane.fold(Msg::Mouse {
        event: MouseEvent::new(MouseKind::Drag(Button::Left), col, row)
            .at_px(to_x as u32, y as u32),
        target: None,
    });
    assert!(
        pane.model.panel_fraction(
            magistr_cockpit::cockpit::flight_divider::FlightDivider::Columns,
            0.65
        ) < was
    );
    pane.fold(Msg::Mouse {
        event: MouseEvent::new(MouseKind::Up(Button::Left), col, row).at_px(to_x as u32, y as u32),
        target: None,
    });
    assert!(pane.model.active_panel().is_none());

    // The next frame puts the native boundary at the pointer, preserving grab offset.
    let _ = pane.paint();
    let [moved_x, _, _, _] = pane
        .frame
        .rect_of(magistr_gui2::comp_workspace::WorkspaceComposition::id(
            Widget::PanelColumns.id(),
        ))
        .expect("the divider is placed");
    assert!(
        (moved_x - (to_x as f32 - dw / 2.0)).abs() < 1.0,
        "{moved_x} vs {dx} less eight cells of {}",
        cell.width()
    );
}

/// The chunk's user report, replayed: a turn taller than the window streams while the
/// viewport follows. The newest row sits flush above the composer, the composer stays
/// on screen with the foot below it, and the stage's scissor — the paint truth, since
/// an overflowed row's layout box legitimately reaches above the stage — keeps every
/// feed pixel off the header band.
#[test]
fn a_long_streaming_turn_keeps_its_tail_above_the_composer() {
    let mut pane = Pane::new();
    let inbox = pane.inbox();
    for line in 0..100 {
        inbox.send(Msg::Agent(AgentEvent::Text(format!("stream row {line}\n"))));
    }
    pane.drain();
    let _ = pane.paint();
    assert!(pane.model.viewport().is_following(), "the window follows");
    assert!(
        pane.model.turn().len() > pane.model.stage_rows(),
        "the turn overflows the window"
    );

    let cell = pane.frame.metrics();
    let viewport_h = f32::from(SIZE.rows) * cell.height();
    let [_, pill_y, _, pill_h] = pane
        .frame
        .rect_of(magistr_gui2::ComposerComposition::PILL)
        .expect("the composer is placed");
    assert!(
        pill_y + pill_h <= viewport_h,
        "the composer is on screen: bottom {} in {viewport_h}",
        pill_y + pill_h
    );

    // The newest visible row lands flush on the feed's bottom edge,
    // which is the composer's top.
    let last = pane
        .model
        .visible_rows()
        .last()
        .expect("a windowed row")
        .text
        .clone();
    let said = pane.frame.says();
    let tail = said
        .iter()
        .filter(|(text, _)| *text == last)
        .map(|(_, rect)| rect[1] + rect[3])
        .fold(f32::MIN, f32::max);
    let [_, stage_y, _, stage_h] = pane
        .frame
        .rect_of(magistr_gui2::RootComposition::STAGE)
        .expect("the stage is placed");
    assert!(
        (tail - (stage_y + stage_h)).abs() < 2.0,
        "the tail is flush at the fold: row bottom {tail}, stage bottom {}",
        stage_y + stage_h
    );
    assert!(
        tail <= pill_y + 1.0,
        "the tail sits above the composer: {tail} vs {pill_y}"
    );

    // The foot still stands under the composer, inside the window.
    assert!(
        said.iter()
            .any(|(_, rect)| rect[1] > pill_y + pill_h && rect[1] + rect[3] <= viewport_h),
        "the foot is on screen"
    );

    // Paint truth for the header band: the overflow is real (rows lay
    // out above the stage) and the one scissor is what stops the ink.
    assert!(
        said.iter().any(|(_, rect)| rect[1] < stage_y),
        "rows overflow the stage's top — the clip has work to do"
    );
    let scissors: Vec<[u32; 4]> = pane
        .frame
        .list()
        .batches
        .iter()
        .filter_map(|batch| batch.scissor)
        .filter(|clip| {
            (clip[1] as f32 - stage_y).abs() < 1.0 && (clip[3] as f32 - stage_h).abs() < 2.0
        })
        .collect();
    assert_eq!(scissors.len(), 1, "the stage has its own scissored batch");
    #[allow(clippy::cast_precision_loss)]
    let clip_top = scissors[0][1] as f32;
    assert!(
        clip_top >= 51.75,
        "the scissor starts below the header band: {clip_top}"
    );
}

/// A pinned (scrolled-up) window anchors at the top and clips at the
/// bottom: the first windowed row sits at the stage's top edge, the
/// overflow now reaches below the fold, and the composer never moves.
#[test]
fn a_pinned_window_anchors_at_the_top_and_clips_at_the_bottom() {
    let mut pane = Pane::new();
    let inbox = pane.inbox();
    for line in 0..100 {
        inbox.send(Msg::Agent(AgentEvent::Text(format!("stream row {line}\n"))));
    }
    pane.drain();
    let _ = pane.paint();
    pane.fold(Msg::Key(KeyEvent::new(KeyCode::PageUp)));
    // Let the scroll's easing settle before measuring the window.
    for _ in 0..240 {
        pane.fold(Msg::Tick);
    }
    let _ = pane.paint();
    assert!(
        !pane.model.viewport().is_following(),
        "the window is pinned"
    );

    let [_, stage_y, _, stage_h] = pane
        .frame
        .rect_of(magistr_gui2::RootComposition::STAGE)
        .expect("the stage is placed");
    let first = pane.model.visible_rows()[0].text.clone();
    let said = pane.frame.says();
    let top = said
        .iter()
        .filter(|(text, _)| *text == first)
        .map(|(_, rect)| rect[1])
        .fold(f32::MAX, f32::min);
    assert!(
        (top - stage_y).abs() < 2.0,
        "the first windowed row anchors at the stage's top: {top} vs {stage_y}"
    );
    assert!(
        said.iter()
            .any(|(_, rect)| rect[1] + rect[3] > stage_y + stage_h),
        "the overflow now reaches below the fold, where the scissor clips it"
    );

    let cell = pane.frame.metrics();
    let viewport_h = f32::from(SIZE.rows) * cell.height();
    let [_, pill_y, _, pill_h] = pane
        .frame
        .rect_of(magistr_gui2::ComposerComposition::PILL)
        .expect("the composer is placed");
    assert!(
        pill_y + pill_h <= viewport_h,
        "the composer holds its place while the window is pinned"
    );
}
