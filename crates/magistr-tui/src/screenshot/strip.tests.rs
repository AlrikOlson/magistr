use std::time::Duration;

use super::*;

use super::super::profile::Profile;
use magistr_cockpit::cockpit::{Msg, Size, update};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};

/// A model that is animating: a turn streaming, so the shimmer runs.
fn moving() -> Model {
    let mut model = Model::new(Size::new(40, 8)).with_theme(Theme::forced(ThemeMode::Dark));
    update(
        &mut model,
        Msg::Agent(magistr_cockpit::agent::AgentEvent::Text(
            "streaming".to_owned(),
        )),
    );
    assert!(model.is_animating(), "the fixture moves");
    model
}

fn shot(model: &Model) -> Pixmap {
    frame(model, &FrameClock::new(0, Duration::ZERO), Profile::Cells).expect("layout succeeds")
}

#[test]
fn a_strip_lays_every_frame_side_by_side_with_a_one_cell_gutter() {
    let mut model = moving();
    let one = shot(&model);
    let strip = strip(
        &mut model,
        &FrameClock::new(0, Duration::ZERO),
        4,
        Profile::Cells,
    )
    .expect("layout succeeds");
    assert_eq!(strip.height(), one.height(), "a strip is one frame tall");
    assert_eq!(
        strip.width(),
        one.width() * 4 + Typeface::CELL_WIDTH * 3,
        "four frames and three gutters wide"
    );
}

#[test]
fn the_first_panel_is_the_frame_before_any_tick() {
    let mut model = moving();
    let before = shot(&model);
    let strip = strip(
        &mut model,
        &FrameClock::new(0, Duration::ZERO),
        3,
        Profile::Cells,
    )
    .expect("layout succeeds");
    for row in 0..before.height() {
        let want = &before.pixels()[(row * before.width()) as usize..][..before.width() as usize];
        let got = &strip.pixels()[(row * strip.width()) as usize..][..before.width() as usize];
        assert_eq!(got, want, "row {row} of the first panel");
    }
}

#[test]
fn a_strip_advances_the_model_one_tick_per_frame() {
    let mut model = moving();
    let _ = strip(
        &mut model,
        &FrameClock::new(0, Duration::ZERO),
        5,
        Profile::Cells,
    )
    .expect("layout succeeds");
    assert_eq!(model.ticks(), 4, "five frames, four steps between them");
}

#[test]
fn the_gutter_is_the_palette_divider() {
    let mut model = moving();
    let one = shot(&model);
    let palette = model.theme().palette();
    let strip = strip(
        &mut model,
        &FrameClock::new(0, Duration::ZERO),
        2,
        Profile::Cells,
    )
    .expect("layout succeeds");
    let x = one.width() + Typeface::CELL_WIDTH / 2;
    let pixel = strip.pixels()[(strip.width() * (one.height() / 2) + x) as usize];
    assert_eq!(
        [pixel.red(), pixel.green(), pixel.blue()],
        [palette.overlay0.r, palette.overlay0.g, palette.overlay0.b],
        "the gutter carries the palette's divider colour"
    );
    assert_eq!(pixel.alpha(), 255, "and is opaque like the frames");
}

#[test]
fn reduced_motion_collapses_the_strip_to_the_still_frame() {
    let mut model =
        moving().with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
    let still = shot(&model);
    let strip = strip(
        &mut model,
        &FrameClock::new(0, Duration::ZERO),
        6,
        Profile::Cells,
    )
    .expect("layout succeeds");
    assert_eq!(strip.width(), still.width(), "one frame wide");
    assert_eq!(strip.data(), still.data(), "and the still frame's pixels");
    assert_eq!(model.ticks(), 0, "no step was taken");
}

#[test]
fn a_strip_of_one_frame_is_that_frame() {
    let mut model = moving();
    let one = shot(&model);
    let strip = strip(
        &mut model,
        &FrameClock::new(0, Duration::ZERO),
        0,
        Profile::Cells,
    )
    .expect("layout succeeds");
    assert_eq!(strip.data(), one.data(), "zero frames still renders one");
}
