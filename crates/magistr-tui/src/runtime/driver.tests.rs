use super::*;
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
    assert!(driver.model.animates(Animation::Motion));
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
fn fractional_retirement_returns_to_the_running_loops_clock_period() {
    let mut driver = driver(model(), false);
    driver.fold(Msg::Loop(LoopEvent::Started {
        iteration: 1,
        max: 2,
    }));
    retire_scroll(&mut driver);
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
