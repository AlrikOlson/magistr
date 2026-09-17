use magistr_cockpit::cockpit::{Model, Motion, Msg, Size, Surface, update};
use magistr_cockpit::theme::{MotionMode, Theme};

use super::*;
use crate::fixture::Fixture;

const SIZE: Size = Size::new(120, 40);

fn tick(model: &mut Model, times: u64) {
    for _ in 0..times {
        let _ = update(model, Msg::Tick);
    }
}

#[test]
fn the_wave_rests_at_both_feet_and_crests_in_the_middle() {
    assert_eq!(Breath::at(0.0).wave(), 0.0);
    assert!((Breath::at(0.5).wave() - 1.0).abs() < 1e-6);
    assert!(Breath::at(1.0).wave().abs() < 1e-6);
    assert!((Breath::at(0.25).wave() - 0.5).abs() < 1e-6);
}

/// A working window breathes on the sky's rhythm and rests at every
/// GLOW_STEPS foot — the frame every settled still is taken at.
#[test]
fn a_working_window_breathes_on_the_glow_steps_and_rests_at_the_foot() {
    let mut model = Fixture::working(SIZE);
    assert!(model.is_working());
    let foot = Breath::of(&model).expect("a working window breathes");
    assert_eq!(foot.wave(), 0.0, "settled on a foot of the wave");
    tick(&mut model, Motion::GLOW_STEPS / 2);
    let crest = Breath::of(&model).expect("still working");
    assert!(crest.wave() > 0.99, "the crest: {}", crest.wave());
    tick(&mut model, Motion::GLOW_STEPS / 2);
    assert_eq!(Breath::of(&model).map(Breath::wave), Some(0.0));
}

/// The breath is the window's, and only a working one's: a terminal,
/// reduced motion, an idle cockpit and a decision screen all answer
/// nothing.
#[test]
fn a_terminal_reduced_motion_idleness_and_a_decision_screen_hold_still() {
    let mut terminal = Fixture::working(SIZE);
    terminal.set_surface(Surface::Terminal);
    assert!(
        Breath::of(&terminal).is_none(),
        "the terminal keeps its contract"
    );

    let reduced =
        Fixture::working(SIZE).with_theme(Theme::default().with_motion(MotionMode::Reduced));
    assert!(
        Breath::of(&reduced).is_none(),
        "reduced motion is structural"
    );

    let idle = Model::new(SIZE).with_surface(Surface::Native);
    assert!(!idle.is_working());
    assert!(Breath::of(&idle).is_none(), "nothing to breathe for");

    let asked = Fixture::mock("06", SIZE).expect("view 06");
    let mut asked = asked;
    asked.set_surface(Surface::Native);
    assert!(asked.moment().is_some(), "a plan ask is up");
    assert!(
        Breath::of(&asked).is_none(),
        "a decision screen holds still"
    );
}

/// The ring the keyframe spreads is its own outline pinned outside the
/// card, by night under the light on the wrapper, by day in ink alone;
/// the foot draws neither, and the wrapper is there either way.
#[test]
fn night_is_a_ring_under_a_light_day_is_the_ring_alone_and_the_foot_is_neither() {
    let lex = Lexicon::of(Medium::Night);
    let card = || Element::surface().width(10.0).height(10.0);
    let rest = Breath::at(0.0).halo(card(), 4.0, Medium::Night, &lex);
    assert!(rest.glow.is_none() && rest.stroke.is_none());
    assert_eq!(rest.count(), 2, "wrapper and card, no ring");
    let ring_of = |root: &Element| {
        let mut ring = None;
        root.each(&mut |_, element| {
            if element.pin[0].is_some() {
                ring = Some((element.pin[0], element.stroke));
            }
        });
        ring
    };
    let night = Breath::at(0.5).halo(card(), 4.0, Medium::Night, &lex);
    let (light, reach) = night.glow.expect("the light");
    assert!(light.a > 0.27 && light.a < 0.29);
    assert!((reach - Breath::REACH_PX).abs() < 1e-3);
    let (out, stroke) = ring_of(&night).expect("the ring");
    let (ink, width) = stroke.expect("the ring is a stroke");
    assert!((width - 2.0 * Breath::RING_WIDTH_PX).abs() < 1e-3);
    assert!(
        out.expect("pinned") < -Breath::RING_PX,
        "spread out past the card"
    );
    assert!(ink.a > 0.2);
    let day = Breath::at(0.5).halo(card(), 4.0, Medium::Day, &Lexicon::of(Medium::Day));
    assert!(day.glow.is_none(), "ink is still by day");
    let (_, stroke) = ring_of(&day).expect("the ring");
    assert!(stroke.expect("a stroke").0.a > 0.5);
}

#[test]
fn the_live_dot_comes_up_out_of_the_ground_with_the_wave() {
    let lex = Lexicon::of(Medium::Night);
    let foot = Breath::at(0.0).dot(Medium::Night, &lex);
    assert_eq!(foot.fill.map(|f| f.a), Some(0.0), "nothing at the foot");
    assert!(foot.glow.is_none());
    let crest = Breath::at(0.5).dot(Medium::Night, &lex);
    assert!(crest.fill.map(|f| f.a).unwrap_or(0.0) > 0.99);
    assert!(crest.glow.is_some());
    let day = Breath::at(0.5).dot(Medium::Day, &Lexicon::of(Medium::Day));
    assert!(day.glow.is_none(), "ink is still by day");
}
