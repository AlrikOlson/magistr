//! motion laws over the blessed views: the app moves in exactly the sixteen ways the
//! design names, each view in the two ways its two columns say, with one hero per
//! screen — and a decision screen holds still.
//!
//! The registry is the cockpit's (`MotionName`, `Model::motions_in_flight`,
//! `Model::motion_hero`); what this suite pins is the *reading* of every fixture
//! through it, in both media, so a view that starts moving in a way the design did not
//! name — or stops moving in a way it did — is a failing row with the view's id on it
//! rather than a reviewer's impression.

use magistr_cockpit::cockpit::{
    Animation, Model, Motion, MotionName, Msg, ShipRun, Size, Surface, update,
};
use magistr_cockpit::input::{MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme};
use magistr_gui2::Medium;
use magistr_gui2::fixture::Fixture;

use MotionName::{Born, Breathe, EdgeRun, Ignite, Radar, ReadyPulse, Sweep, Twinkle};

/// The cockpit grid the fixtures are built on.
fn size() -> Size {
    Size::new(120, 40)
}

/// `model` on a window in `medium`, awake: an input has just landed and
/// the chrome's 150 ms fade is over, so the reading is of the screen a
/// person is looking at — not the one that exhaled into rest, where
/// nothing moves by design.
fn awake(model: Model, medium: Medium) -> Model {
    let mut model = Fixture::with_pixels(
        model
            .with_surface(Surface::Native)
            .with_theme(Theme::forced(medium.mode())),
    );
    let _ = update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, 0, 0),
            target: None,
        },
    );
    for _ in 0..Motion::CHROME_FADE_STEPS {
        let _ = update(&mut model, Msg::Tick);
    }
    model
}

/// The same model under reduced motion.
fn reduced(model: Model, medium: Medium) -> Model {
    Fixture::with_pixels(
        model
            .with_surface(Surface::Native)
            .with_theme(Theme::forced(medium.mode()).with_motion(MotionMode::Reduced)),
    )
}

/// The views that wait on a person — ask, review, the chapter, settings, the inbox,
/// search, a bad day, and Birth's ground, interview and first light — which the design
/// says hold still.
const DECISION_SCREENS: [&str; 10] = ["06", "07", "09", "10", "12", "13", "14", "F1", "F3", "F5"];

/// What each view moves, awake, by medium — section III read per view:
///
/// * the Galaxy (01) and the Sky (03, 04) breathe on their working
///   thing (the live cluster, the executing star) — the hero — with
///   the shipped stars twinkling beside it at night and never by day,
///   and the ready star pulsing where the awake sky has one;
/// * the Session (05) at iteration 4 of 12 is the loop at work — the
///   breath is its hero (the streaming sweep is a turn's, and the
///   capture's turn has settled);
/// * the settled ship moment (08) keeps that loop's breath and the
///   texture: the twinkle and the pulse of the star the completion
///   made ready (the flare itself is [`Fixture::ship_08`]'s, below);
/// * the survey (F2) is the radar, the charting (F4) is born;
/// * the Trace (11) and every decision screen hold still.
fn expected(view: &str, medium: Medium) -> Vec<MotionName> {
    let night = medium == Medium::Night;
    let with_twinkle = |mut names: Vec<MotionName>| {
        if night {
            names.push(Twinkle);
        }
        MotionName::ALL
            .into_iter()
            .filter(|name| names.contains(name))
            .collect()
    };
    match view {
        "01" | "03" => with_twinkle(vec![Breathe]),
        "04" | "08" => with_twinkle(vec![Breathe, ReadyPulse]),
        "05" => vec![Breathe],
        "F2" => vec![Radar],
        "F4" => vec![Born],
        _ => Vec::new(),
    }
}

/// Every view moves as section III's two columns say, has one hero,
/// and — whenever it moves at all — declares the tick that moves it.
#[test]
fn every_view_moves_as_the_design_names_and_has_one_hero() {
    let mut misses = Vec::new();
    let mut heroes = Vec::new();
    for view in Fixture::VIEWS {
        for medium in Medium::ALL {
            let Some(model) = Fixture::mock(view, size()) else {
                continue;
            };
            let model = awake(model, medium);
            let actual = model.motions_in_flight();
            let expected = expected(view, medium);
            if actual != expected {
                misses.push(format!(
                    "{medium}-{view}: the design names {expected:?}, in flight {actual:?}"
                ));
            }
            let hero = model.motion_hero();
            assert_eq!(hero, MotionName::hero(actual.iter().copied()));
            heroes.push(format!("{medium}-{view}: {hero:?}"));
            if !actual.is_empty() {
                assert!(
                    model.animates(Animation::Motion),
                    "{medium}-{view}: a motion in flight declares the tick"
                );
            }
            for name in &actual {
                assert!(
                    name.by(medium.mode()).is_some(),
                    "{medium}-{view}: {name:?} has no column in this medium"
                );
            }
            if medium == Medium::Day {
                assert!(
                    !actual.contains(&Twinkle),
                    "{medium}-{view}: ink never twinkles"
                );
            }
        }
    }
    assert!(
        misses.is_empty(),
        "{}\n\nheroes:\n{}",
        misses.join("\n"),
        heroes.join("\n")
    );
}

/// A decision screen holds still: nothing is in flight but, at most, the caret, so
/// there is no hero.
#[test]
fn decision_screens_hold_still() {
    for view in DECISION_SCREENS {
        for medium in Medium::ALL {
            let model = awake(
                Fixture::mock(view, size()).expect("a decision view has a fixture"),
                medium,
            );
            let actual = model.motions_in_flight();
            assert!(
                actual.iter().all(|name| *name == MotionName::Caret),
                "{medium}-{view}: a decision screen holds still, in flight {actual:?}"
            );
            assert_eq!(model.motion_hero(), None, "{medium}-{view}");
        }
    }
}

/// The ship moment's hero is the ignite, then the edge-run, and then
/// the sky is texture again — the flare goes out of existence rather
/// than pinning at its end.
#[test]
fn the_ship_moment_ignites_then_runs_the_edge_then_settles() {
    for medium in Medium::ALL {
        let mut model = awake(Fixture::ship_08(size()), medium);
        assert_eq!(model.motion_hero(), Some(Ignite), "{medium}");
        assert!(
            model.moves(ReadyPulse),
            "{medium}: the star the completion made ready pulses under the flare"
        );
        for _ in 0..ShipRun::IGNITE_STEPS {
            let _ = update(&mut model, Msg::Tick);
        }
        assert_eq!(model.motion_hero(), Some(EdgeRun), "{medium}");
        for _ in 0..ShipRun::EDGE_RUN_STEPS {
            let _ = update(&mut model, Msg::Tick);
        }
        let after = model.motion_hero();
        assert!(
            after.is_none_or(MotionName::is_ambient),
            "{medium}: the moment is over, only texture remains: {after:?}"
        );
        assert!(!model.moves(Ignite) && !model.moves(EdgeRun), "{medium}");
    }
}

/// The working session breathes — the one hero of a stage waiting on
/// the agent — in both media.
#[test]
fn the_working_stage_breathes() {
    for medium in Medium::ALL {
        let model = awake(Fixture::working(size()), medium);
        assert!(model.moves(Breathe), "{medium}");
        assert!(
            model
                .motion_hero()
                .is_some_and(|hero| hero == Breathe || hero == Sweep),
            "{medium}: {:?}",
            model.motion_hero()
        );
    }
}

/// Reduced motion answers the empty set on every view, structurally:
/// nothing is in flight and nothing asks for a tick.
#[test]
fn reduced_motion_answers_the_empty_set_on_every_view() {
    let mut models: Vec<(String, Model)> = Vec::new();
    for view in Fixture::VIEWS {
        for medium in Medium::ALL {
            if let Some(model) = Fixture::mock(view, size()) {
                models.push((format!("{medium}-{view}"), reduced(model, medium)));
            }
        }
    }
    for medium in Medium::ALL {
        models.push((
            format!("{medium}-ship"),
            reduced(Fixture::ship_08(size()), medium),
        ));
        models.push((
            format!("{medium}-working"),
            reduced(Fixture::working(size()), medium),
        ));
    }
    for (name, model) in models {
        assert!(
            model.motions_in_flight().is_empty(),
            "{name}: {:?}",
            model.motions_in_flight()
        );
        assert_eq!(model.motion_hero(), None, "{name}");
        assert!(
            model.animations().is_empty(),
            "{name}: nothing asks for a tick"
        );
    }
}
