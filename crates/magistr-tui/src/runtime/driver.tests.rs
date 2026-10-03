use super::*;

#[test]
fn a_closed_command_owner_reports_stop_without_claiming_it_stopped() {
    let mut driver = driver(model(), false);
    driver.fold(Msg::Agent(magistr_application::sessions::AgentEvent::Text(
        "Working".into(),
    )));
    driver.fold(Msg::Paste("Keep @src/main.rs".into()));
    driver.fold(Msg::Key(
        KeyEvent::new(KeyCode::Char('c')).with(magistr_cockpit::input::Modifiers::CTRL),
    ));
    assert!(driver.model.agent_activity().active);
    assert_ne!(driver.model.agent_activity().status, "Stopping");
    assert_eq!(driver.model.prompt().text(), "Keep @src/main.rs");
    assert!(
        driver
            .model
            .turn()
            .lines()
            .iter()
            .any(|line| line.text().contains("could not stop"))
    );
}

use magistr_cockpit::capability::Mode;
use magistr_cockpit::cockpit::{LoopEvent, Size};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit::turn::{Viewport, Voice};
use magistr_cockpit::widget_id::WidgetId;

fn model() -> Model {
    let mut model = Model::new(Size::new(144, 50));
    update(
        &mut model,
        Msg::Mode(Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    for index in 0..100 {
        model
            .turn_mut()
            .push_text(Voice::Agent, &format!("Saved record {index}\n"));
    }
    update(&mut model, Msg::FrameDue);
    for _ in 0..24 {
        update(&mut model, Msg::Tick);
    }
    assert!(!model.is_animating());
    model
}

#[test]
fn a_closed_command_owner_restores_messages_in_both_views() {
    for roadmap in [false, true] {
        let mut driver = driver(model(), false);
        if roadmap {
            mouse(
                &mut driver,
                MouseKind::Down(Button::Left),
                Some(Widget::TabRoadmap.id()),
            );
            mouse(
                &mut driver,
                MouseKind::Up(Button::Left),
                Some(Widget::TabRoadmap.id()),
            );
        }
        driver.fold(Msg::Paste("Please add recovery checks".into()));
        mouse(
            &mut driver,
            MouseKind::Down(Button::Left),
            Some(Widget::PromptAction.id()),
        );
        mouse(
            &mut driver,
            MouseKind::Up(Button::Left),
            Some(Widget::PromptAction.id()),
        );
        assert_eq!(driver.model.prompt().text(), "Please add recovery checks");
        assert_ne!(
            driver.model.turn_state(),
            magistr_cockpit::cockpit::TurnState::Streaming
        );
        assert!(
            driver
                .model
                .turn()
                .lines()
                .iter()
                .any(|line| line.text().contains("connection closed"))
                || driver
                    .model
                    .roadmap_requests()
                    .last()
                    .and_then(|request| request.error.as_deref())
                    .is_some_and(|error| error.contains("connection closed"))
        );
    }
}

fn driver(model: Model, flat: bool) -> Driver {
    let (commands, _) = tokio::sync::mpsc::unbounded_channel();
    Driver::new(model, Screen::new(144, 50), commands, None, false, flat)
}

fn mouse(driver: &mut Driver, kind: MouseKind, target: Option<WidgetId>) {
    driver.fold(Msg::Mouse {
        event: MouseEvent::new(kind, 1, 1),
        target,
    });
}

/// Retargeting a scroll between ticks makes its glide finish between ticks.
/// No elapsed wall time is needed to reproduce that boundary.
fn retire_scroll(driver: &mut Driver) {
    driver.fold(Msg::Key(KeyEvent::new(KeyCode::Home)));
    driver.fold(Msg::Phase(0.25));
    driver.fold(Msg::Key(KeyEvent::new(KeyCode::Home)));
    for _ in 0..Viewport::GLIDE_STEPS {
        driver.tick();
    }
    assert!(
        driver.model.animates(Animation::Motion),
        "now={} scrolling={} top={} total={} motion={:?}",
        driver.model.now(),
        driver.model.is_scrolling(),
        driver.model.top_row(),
        driver.model.turn().len(),
        driver.model.motions_in_flight()
    );
    driver.fold(Msg::Phase(0.5));
    assert!(!driver.model.animates(Animation::Motion));
    assert!(
        driver.scheduler.is_animating(),
        "the whole tick stays booked"
    );
    assert_eq!(driver.model.now().fract(), 0.5);
}

#[test]
fn seeded_animation_is_scheduled_before_any_new_model_message() {
    let mut model = model();
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollUp, 1, 1),
            target: Some(Widget::Stage.id()),
        },
    );
    assert!(model.animates(Animation::Motion));
    let before_start = Instant::now();
    let mut driver = driver(model, false);
    assert!(driver.scheduler.is_animating());
    assert!(driver.interpolates());
    assert_eq!(
        driver.scheduler.period(Instant::now()),
        Animation::Motion.period()
    );
    assert!(!driver.scheduler.tick_due(before_start));
    assert!(
        driver
            .scheduler
            .tick_due(Instant::now() + Animation::Motion.period())
    );
    driver.tick();
    assert_eq!(driver.model.ticks(), 25);
}

#[test]
fn fractional_retirement_finishes_the_tick_before_the_next_gesture() {
    let mut driver = driver(model(), false);
    retire_scroll(&mut driver);
    let fraction = driver.model.now();
    driver.tick();
    assert!(driver.model.now() > fraction);
    assert_eq!(driver.model.now().fract(), 0.0);
    assert!(!driver.scheduler.is_animating());
    driver.scheduler.painted();
    assert!(!driver.scheduler.wants_frame());

    mouse(&mut driver, MouseKind::ScrollDown, Some(Widget::Stage.id()));
    let restart = driver.model.now();
    driver.fold(Msg::Phase(0.0));
    assert_eq!(
        driver.model.now(),
        restart,
        "a new interval cannot rewind the gesture"
    );
    assert!(driver.scheduler.is_animating());
    assert!(driver.interpolates());
}

#[test]
fn clearing_a_fractionally_retired_stamp_does_not_leave_the_clock_running() {
    let mut driver = driver(model(), false);
    retire_scroll(&mut driver);
    driver.fold(Msg::Key(KeyEvent::new(KeyCode::Home)));
    assert!(!driver.model.is_animating());
    assert!(driver.scheduler.is_animating());
    driver.tick();
    driver.scheduler.painted();
    assert_eq!(driver.model.now().fract(), 0.0);
    assert!(!driver.scheduler.wants_frame());
}

#[test]
fn reducing_motion_after_fractional_retirement_stops_the_booked_tick() {
    let mut driver = driver(model(), false);
    retire_scroll(&mut driver);
    mouse(
        &mut driver,
        MouseKind::Down(Button::Left),
        Some(Widget::Commands.id()),
    );
    for letter in "settings".chars() {
        driver.fold(Msg::Key(KeyEvent::new(KeyCode::Char(letter))));
    }
    driver.fold(Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(driver.model.settings().is_some());
    mouse(
        &mut driver,
        MouseKind::Down(Button::Left),
        Some(Widget::settings_switch(2)),
    );
    assert!(!driver.model.theme().motion().is_full());
    assert!(!driver.interpolates());
    driver.tick();
    driver.scheduler.painted();
    assert_eq!(driver.model.now().fract(), 0.0);
    assert!(!driver.scheduler.wants_frame());
}

#[test]
fn fractional_retirement_returns_to_the_paused_loops_clock_period() {
    let mut driver = driver(model(), false);
    driver.fold(Msg::Loop(LoopEvent::Started {
        iteration: 1,
        max: 2,
    }));
    driver.fold(Msg::Loop(LoopEvent::Paused("Review the result".into())));
    // Let the pause note settle. A paused loop consumes Home, so use the
    // wheel to retarget a scroll between ticks.
    for _ in 0..24 {
        driver.tick();
    }
    mouse(&mut driver, MouseKind::ScrollUp, Some(Widget::Stage.id()));
    driver.fold(Msg::Phase(0.25));
    mouse(&mut driver, MouseKind::ScrollUp, Some(Widget::Stage.id()));
    for _ in 0..Viewport::GLIDE_STEPS {
        driver.tick();
    }
    assert!(driver.model.animates(Animation::Motion));
    driver.fold(Msg::Phase(0.5));
    assert!(!driver.model.animates(Animation::Motion));
    assert_eq!(
        driver.model.animations().period(),
        Some(Animation::Clock.period())
    );
    assert_eq!(
        driver.scheduler.period(Instant::now()),
        Animation::Motion.period()
    );
    let before_tick = Instant::now();
    driver.tick();
    assert!(
        !driver
            .scheduler
            .tick_due(before_tick + Animation::Motion.period())
    );
    assert!(
        driver
            .scheduler
            .tick_due(Instant::now() + Animation::Clock.period())
    );
    assert!(!driver.interpolates());
    assert_eq!(driver.model.now().fract(), 0.0);
}

#[test]
fn waiting_for_a_response_samples_at_a_lower_rate_than_gestures() {
    let mut driver = driver(model(), false);
    driver.fold(Msg::Agent(
        magistr_application::sessions::AgentEvent::UserText("Review the plan".into()),
    ));
    for _ in 0..24 {
        driver.tick();
    }
    assert!(driver.model.agent_activity().active);
    assert!(driver.model.is_animating());
    assert!(!driver.interpolates());
    assert_eq!(
        driver.frame_period(Instant::now()),
        Duration::from_millis(32)
    );
    driver.fold(Msg::Phase(0.5));
    assert_eq!(driver.model.now().fract(), 0.5);
    mouse(&mut driver, MouseKind::ScrollUp, Some(Widget::Stage.id()));
    assert!(driver.interpolates());
}

#[test]
fn flat_and_reduced_motion_do_not_request_interpolated_frames() {
    let mut plain = driver(model(), true);
    mouse(&mut plain, MouseKind::ScrollUp, Some(Widget::Stage.id()));
    assert!(plain.model.animates(Animation::Motion));
    assert!(!plain.interpolates());
    assert_eq!(
        plain.frame_period(Instant::now()),
        Animation::Motion.period()
    );

    let theme = magistr_cockpit::theme::Theme::default()
        .with_motion(magistr_cockpit::theme::MotionMode::Reduced);
    let mut reduced = driver(model().with_theme(theme), false);
    mouse(&mut reduced, MouseKind::ScrollUp, Some(Widget::Stage.id()));
    assert!(!reduced.interpolates());
    assert!(!reduced.scheduler.is_animating());
}

#[test]
fn terminal_replies_reach_the_model_as_capabilities_without_commands_or_input() {
    use crate::input::Decoder;
    use magistr_cockpit::render::Color;
    let (commands, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut driver = Driver::new(
        Model::new(Size::new(80, 24)),
        Screen::new(80, 24),
        commands,
        None,
        false,
        false,
    );
    let mut decoder = Decoder::new();
    for byte in b"\x1b]11;#eff1f5\x07\x1b[?3u\x1b[4;480;800t" {
        let mut events = Vec::new();
        decoder.feed(&[*byte], &mut events);
        for event in events {
            driver.event(InputEvent::Event(event));
        }
    }
    assert_eq!(driver.model.background(), Some(Color::Rgb(239, 241, 245)));
    assert_eq!(
        driver.model.theme().mode(),
        magistr_cockpit::theme::ThemeMode::Light
    );
    assert!(driver.model.reports_key_releases());
    assert_eq!(driver.model.cell_pixels(), Some((10, 20)));
    assert_eq!(driver.model.turn().cell_size(), driver.model.cell_size());
    assert_eq!(driver.model.inputs(), 0);
    assert!(driver.model.prompt().is_empty());
    assert!(driver.model.recent().next().is_none());
    assert!(
        receiver.try_recv().is_err(),
        "query replies cannot request effects"
    );
}

#[test]
fn queued_mouse_motion_cannot_hold_back_a_message_or_the_next_frame() {
    use crate::harness::{FakeTerminal, Script};

    let mut driver = driver(Model::new(Size::new(144, 50)), false);
    let (input, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (messages, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    for index in 0..4096 {
        input
            .send(InputEvent::Event(Event::Mouse(MouseEvent::new(
                MouseKind::Move,
                index % 144,
                1,
            ))))
            .unwrap();
    }
    messages
        .send(Msg::Paste("Keep the screen responsive".into()))
        .unwrap();

    driver.drain(&mut rx, &mut inbox);

    assert!(
        !rx.is_empty(),
        "drawing cannot wait for the entire input backlog"
    );
    assert_eq!(driver.model.prompt().text(), "Keep the screen responsive");
    let mut terminal = FakeTerminal::new(Size::new(144, 50), Script::new());
    let capture = terminal.capture();
    driver.paint(&mut terminal).unwrap();
    assert!(capture.count(b"Keep the screen responsive") > 0);
}

#[test]
fn queued_messages_cannot_hold_back_the_next_frame() {
    let mut driver = driver(Model::new(Size::new(144, 50)), false);
    let (_input, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (messages, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    for _ in 0..4096 {
        messages.send(Msg::Focus(true)).unwrap();
    }

    driver.drain(&mut rx, &mut inbox);

    assert!(
        !inbox.is_empty(),
        "a live feed must leave time to draw and check shutdown"
    );
    assert!(driver.scheduler.wants_frame());
}

#[test]
fn consecutive_hover_positions_coalesce_without_reordering_other_input() {
    let mut driver = driver(Model::new(Size::new(144, 50)), false);
    let (input, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (_messages, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    for (kind, col) in [
        (MouseKind::Move, 1),
        (MouseKind::Move, 2),
        (MouseKind::Down(Button::Left), 2),
        (MouseKind::Drag(Button::Left), 3),
        (MouseKind::Drag(Button::Left), 4),
        (MouseKind::Up(Button::Left), 4),
        (MouseKind::Move, 5),
        (MouseKind::Move, 6),
    ] {
        input
            .send(InputEvent::Event(Event::Mouse(MouseEvent::new(
                kind, col, 15,
            ))))
            .unwrap();
    }
    while !rx.is_empty() {
        driver.drain(&mut rx, &mut inbox);
    }
    assert_eq!(
        driver.model.inputs(),
        6,
        "both drags and the latest hover positions reach the model"
    );
    let positions: Vec<_> = driver
        .model
        .recent()
        .filter_map(|msg| match msg {
            Msg::Mouse { event, .. } => Some((event.kind, event.col)),
            _ => None,
        })
        .collect();
    assert_eq!(
        positions,
        [
            (MouseKind::Down(Button::Left), 2),
            (MouseKind::Up(Button::Left), 4),
        ]
    );
}

#[tokio::test]
async fn shutdown_and_painting_remain_live_with_queued_mouse_motion() {
    use crate::harness::{FakeTerminal, Script};

    let mut driver = driver(Model::new(Size::new(144, 50)), false);
    let (input, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (_messages, mut inbox) = tokio::sync::mpsc::unbounded_channel();
    for index in 0..16384 {
        input
            .send(InputEvent::Event(Event::Mouse(MouseEvent::new(
                MouseKind::Move,
                index % 144,
                1,
            ))))
            .unwrap();
    }
    let mut terminal = FakeTerminal::new(Size::new(144, 50), Script::new());
    let capture = terminal.capture();
    let first_frame = capture.clone();
    let quit = CancellationToken::new();
    let cancellation = quit.clone();
    let producer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while first_frame.count(b"\x1b[?2026h") == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        cancellation.cancel();
    });

    driver
        .run(&mut terminal, &mut rx, &mut inbox, Some(&quit))
        .await
        .unwrap();
    producer.join().unwrap();

    assert!(
        !rx.is_empty(),
        "shutdown must not wait for input to become idle"
    );
    assert!(
        capture.count(b"\x1b[?2026h") > 0,
        "the busy cockpit still draws"
    );
}

#[test]
fn resize_recomputes_window_fallback_but_preserves_a_direct_measurement() {
    use crate::input::Reply;
    let mut driver = driver(Model::new(Size::new(80, 24)), false);
    driver.event(InputEvent::Event(Event::Reply(Reply::WindowPixels {
        width: 800,
        height: 480,
    })));
    assert_eq!(driver.model.cell_pixels(), Some((10, 20)));
    driver.event(InputEvent::Resize(Size::new(160, 48)));
    assert_eq!(driver.model.cell_pixels(), Some((5, 10)));
    assert_eq!(driver.model.turn().cell_size(), driver.model.cell_size());
    driver.event(InputEvent::Event(Event::Reply(Reply::CellPixels {
        width: 9,
        height: 18,
    })));
    driver.event(InputEvent::Resize(Size::new(80, 24)));
    assert_eq!(driver.model.cell_pixels(), Some((9, 18)));
    driver.event(InputEvent::Event(Event::Reply(Reply::WindowPixels {
        width: 1600,
        height: 960,
    })));
    assert_eq!(driver.model.cell_pixels(), Some((9, 18)));
    driver.event(InputEvent::Event(Event::Reply(Reply::CellPixels {
        width: 0,
        height: 18,
    })));
    assert_eq!(
        driver.model.cell_pixels(),
        None,
        "an invalid direct reply retains precedence"
    );
    assert_eq!(
        driver.model.cell_size(),
        magistr_cockpit::pixel::CellSize::DEFAULT
    );
}

#[test]
fn keyboard_flags_are_interpreted_only_by_the_terminal_runtime() {
    use crate::input::Reply;
    let mut driver = driver(Model::new(Size::new(80, 24)), false);
    for (flags, reports) in [
        (0, false),
        (1, false),
        (2, true),
        (3, true),
        (255, true),
        (1, false),
    ] {
        driver.event(InputEvent::Event(Event::Reply(Reply::KeyboardFlags(flags))));
        assert_eq!(driver.model.reports_key_releases(), reports);
    }
}

#[test]
fn rio_cell_mouse_reaches_visible_control_edges_across_font_sizes() {
    use crate::input::Decoder;
    use magistr_cockpit::layout::PxRect;
    use magistr_cockpit::pixel::CellSize;
    let mut tested = 0;
    let mut edges = 0;
    for (width, height) in [(10, 20), (16, 33), (22, 47)] {
        let mut driver = driver(model(), false);
        driver.fold(Msg::CellSize(Some(CellSize::new(width, height))));
        let root = view(&driver.model, &FrameClock::new(0, Duration::ZERO));
        let resolved = driver
            .surfaces
            .layout(&root, &driver.model, &mut driver.tree)
            .unwrap();
        for widget in [Widget::Commands, Widget::PromptFiles, Widget::PromptAction] {
            let Some(place) = resolved
                .placed()
                .iter()
                .find(|place| place.id == Some(widget.id()))
            else {
                continue; // Disabled controls have no interactive bounds.
            };
            let rect = place.px;
            let col = ((rect.x + rect.width * 0.5) / f32::from(width)) as u16;
            for row in [
                ((rect.y + rect.height * 0.5) / f32::from(height)) as u16,
                (rect.y / f32::from(height)) as u16,
                ((rect.bottom() - 0.01) / f32::from(height)) as u16,
            ] {
                let region = PxRect::new(
                    f32::from(col) * f32::from(width),
                    f32::from(row) * f32::from(height),
                    f32::from(width),
                    f32::from(height),
                );
                let center = (
                    f64::from(region.x + region.width * 0.5),
                    f64::from(region.y + region.height * 0.5),
                );
                let edge = !rect.contains(center.0, center.1);
                // A cell spanning two controls cannot locate a sub-cell click.
                // Literal coverage and overlay tests pin that case separately.
                let ambiguous = resolved.placed().iter().any(|other| {
                    other.id != Some(widget.id())
                        && PointerShape::over(other.id) == PointerShape::Hand
                        && !other.px.intersection(region).is_empty()
                });
                if rect.intersection(region).height < 2.0 || (edge && ambiguous) {
                    continue;
                }
                let report = format!("\x1b[<35;{};{}M", col + 1, row + 1);
                let mut decoder = Decoder::new();
                let mut events = Vec::new();
                decoder.feed(report.as_bytes(), &mut events);
                assert_eq!(events.len(), 1);
                for event in events {
                    driver.event(InputEvent::Event(event));
                }
                assert_eq!(
                    driver.model.hovered(),
                    Some(widget.id()),
                    "{widget:?} at {width}x{height}, cell {col},{row}, bounds {rect:?}"
                );
                tested += 1;
                edges += usize::from(edge);
            }
        }
    }
    assert!(tested >= 3, "exercise different measured font sizes");
    assert!(edges > 0, "exercise painted edges beyond cell centers");
}

#[test]
fn rio_cell_mouse_clicks_a_visible_header_edge_and_preserves_the_draft() {
    use crate::input::Decoder;
    let mut driver = driver(model(), false);
    driver.fold(Msg::Paste("Keep this draft".into()));
    let mut decoder = Decoder::new();
    let mut events = Vec::new();
    decoder.feed(b"\x1b[6;20;10t\x1b[<0;137;1M", &mut events);
    for event in events {
        driver.event(InputEvent::Event(event));
    }
    assert!(
        driver.model.palette().is_some(),
        "the visible Commands edge opens its menu"
    );
    assert_eq!(driver.model.prompt().text(), "Keep this draft");
}
