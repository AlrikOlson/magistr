//! Stills of each a5 composition, in both media.
//!
//! One picture per composition per medium, each mapped from a real cockpit model built
//! through the public `update` — the named, reviewable form of the acceptance
//! criterion. The whole-sink goldens hold the same arrangements in page context; these
//! isolate each composition so a regression names its module.

use std::path::PathBuf;

use magistr_cockpit::cockpit::{Model, Motion, Msg, Size, StageTab, Surface, update};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::golden::Golden;
use magistr_cockpit::harness::fixtures;
use magistr_cockpit::input::{KeyCode, KeyEvent};
use magistr_gui2::comp_birth::BirthComposition;
use magistr_gui2::comp_brief::BriefComposition;
use magistr_gui2::comp_composer::ComposerComposition;
use magistr_gui2::comp_feed::FeedComposition;
use magistr_gui2::comp_header::HeaderComposition;
use magistr_gui2::comp_moment::MomentComposition;
use magistr_gui2::comp_rail::RailComposition;
use magistr_gui2::comp_report::ReportComposition;
use magistr_gui2::comp_sky::SkyComposition;
use magistr_gui2::comp_term_dock::TermDockComposition;
use magistr_gui2::fixture::Fixture;
use magistr_gui2::{Element, ElementLayout, ElementState, Medium, TextEngine, atom_still};

const SIZE: Size = Size::new(120, 40);
const CELL: (f32, f32) = (9.0, 20.0);
const WIDTH: u32 = 620;

fn golden() -> Golden {
    Golden::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("comp"),
    )
}

/// `element` rasterized at its natural height over [`WIDTH`], on the
/// medium's own ground.
fn still(element: Element, medium: Medium, name: &str, golden: &Golden) -> Result<(), String> {
    let mut text = TextEngine::embedded();
    let wrapped = Element::surface().align_start().child(element);
    #[allow(clippy::cast_precision_loss)]
    let layout =
        ElementLayout::compute(&wrapped, WIDTH as f32, 65536.0, &mut text).expect("layout");
    let [_, y, _, h] = layout.rect(1);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = ((y + h).ceil() as u32).max(1);
    let pixmap = atom_still(&wrapped, &ElementState::default(), medium, WIDTH, height)
        .expect("the still renders");
    golden
        .check(name, &pixmap)
        .map_err(|error| error.to_string())
}

/// A window opened after hours away, its Brief's deal landed.
fn briefed() -> Model {
    let mut model = Model::new(SIZE)
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::local(fixtures::frame(
            "roadmap",
            "chunk_completed",
            serde_json::json!({
                "chunk": {"id": "e10-fleet", "name": "e10-fleet", "status": "done"}
            }),
        ))),
    );
    let mut model = model.with_away(std::time::Duration::from_secs(6 * 3600 + 12 * 60));
    for _ in 0..(Motion::RISE_STAGGER * 4 + Motion::ARRIVAL_STEPS + 1) {
        update(&mut model, Msg::Tick);
    }
    model
}

/// View 03's sky, native and idled to rest.
fn resting() -> Model {
    let mut model = Fixture::with_pixels(Fixture::mock("03", SIZE).expect("view 03"));
    model.set_surface(Surface::Native);
    for _ in 0..(Motion::REST_STEPS + Motion::CHROME_FADE_STEPS) {
        update(&mut model, Msg::Tick);
    }
    model
}

#[test]
fn every_composition_is_a_golden_in_both_media() {
    let golden = golden();
    let mut failures = Vec::new();
    let mut capture = |element, medium, name: &str, golden: &Golden| {
        if let Err(error) = still(element, medium, name, golden) {
            failures.push(error);
        }
    };
    let mut session = Fixture::mock("05", SIZE).expect("view 05");
    session.set_surface(Surface::Native);
    // The rail with the pointer resting on cp-3's row — the hit registry's target on
    // the last move — so its Restore · Fork are revealed on the raised ground.
    let mut revealed = session.clone();
    let below = revealed.rail().spine().len() - 2;
    update(
        &mut revealed,
        Msg::Mouse {
            event: magistr_cockpit::input::MouseEvent::new(
                magistr_cockpit::input::MouseKind::Move,
                100,
                10,
            ),
            target: Some(magistr_cockpit::cockpit::Widget::spine_row(below)),
        },
    );
    let offered = Fixture::rail_offer(SIZE);
    let idle = Fixture::rail_idle(SIZE);
    let turn = Fixture::turn(SIZE);
    let cards = Fixture::cards(Size::new(120, 400));
    let mut typed = Model::new(SIZE);
    for key in "allow, but stop if the gates fail".chars() {
        update(&mut typed, Msg::Key(KeyEvent::new(KeyCode::Char(key))));
    }
    let briefed = briefed();
    let asked = Fixture::mock("06", SIZE).expect("view 06");
    let mut reviewing = Fixture::mock("07", SIZE).expect("view 07");
    reviewing.set_surface(Surface::Native);
    let resting = resting();
    let shipped = Fixture::ship_08(SIZE);
    let shelved = Fixture::shelved(SIZE);
    let following = Fixture::pane_following(SIZE);
    let scrolled = Fixture::pane_scrolled(SIZE);
    let finished = Fixture::pane_finished(SIZE);
    let docked = Fixture::pane_docked(SIZE);
    let killing = Fixture::pane_killing(SIZE);
    let killed = Fixture::pane_killed(SIZE);
    let maximized = Fixture::pane_maximized(SIZE);
    let ground = Fixture::birth_f1(SIZE);
    let surveyed = Fixture::birth_f2(SIZE);
    let interviewed = Fixture::birth_f3(SIZE);
    // The charting with its stars born: a still of the drawn proposal.
    let mut charted = Fixture::birth_f4(SIZE);
    for _ in 0..(Fixture::SETTLE_STEPS * 2) {
        update(&mut charted, Msg::Tick);
    }
    for medium in [Medium::Night, Medium::Day] {
        let tag = match medium {
            Medium::Night => "night",
            Medium::Day => "day",
        };
        capture(
            HeaderComposition::el(&session, medium),
            medium,
            &format!("header-{tag}"),
            &golden,
        );
        capture(
            RailComposition::el(&session, medium, CELL.0),
            medium,
            &format!("rail-{tag}"),
            &golden,
        );
        capture(
            RailComposition::el(&revealed, medium, CELL.0),
            medium,
            &format!("rail-revealed-{tag}"),
            &golden,
        );
        // The idle rail: the two starts where the running rail wears the loop's verbs.
        capture(
            RailComposition::el(&idle, medium, CELL.0),
            medium,
            &format!("rail-idle-{tag}"),
            &golden,
        );
        capture(
            RailComposition::el(&offered, medium, CELL.0),
            medium,
            &format!("rail-offer-{tag}"),
            &golden,
        );
        capture(
            ComposerComposition::el(&typed, medium).width(WIDTH as f32),
            medium,
            &format!("composer-{tag}"),
            &golden,
        );
        capture(
            FeedComposition::el(&turn, medium, CELL),
            medium,
            &format!("feed-{tag}"),
            &golden,
        );
        // Every card open: one detail per verb.
        capture(
            FeedComposition::el(&cards, medium, CELL),
            medium,
            &format!("cards-{tag}"),
            &golden,
        );
        // The terminal pane: a run card open on its streaming screen — following the
        // tail with the cursor on it, held two notches up with the thumb and the pill,
        // and settled after the exit with the cursor gone. The held kill: the head
        // filling red, then the pane after the half second — frozen, named the human's.
        for (state, model) in [
            ("following", &following),
            ("scrolled", &scrolled),
            ("finished", &finished),
            ("killing", &killing),
            ("killed", &killed),
        ] {
            capture(
                FeedComposition::el(model, medium, CELL),
                medium,
                &format!("pane-{state}-{tag}"),
                &golden,
            );
        }
        // The terminal dock: the same run's screen raised a third of the stage, and
        // maximized.
        for (state, model) in [("dock", &docked), ("dock-max", &maximized)] {
            capture(
                TermDockComposition::el(model, medium).expect("raised"),
                medium,
                &format!("{state}-{tag}"),
                &golden,
            );
        }
        capture(
            Element::surface()
                .height(220.0)
                .child(SkyComposition::el(&resting, medium)),
            medium,
            &format!("sky-rest-{tag}"),
            &golden,
        );
        capture(
            Element::surface()
                .height(140.0)
                .child(SkyComposition::el(&shipped, medium)),
            medium,
            &format!("sky-shipped-{tag}"),
            &golden,
        );
        // The whole Sky at the still's width: the card is pinned off the
        // field's right edge, so the field has to be as wide as the page.
        #[allow(clippy::cast_precision_loss)]
        capture(
            Element::surface()
                .width(WIDTH as f32)
                .height(460.0)
                .child(SkyComposition::el(&shelved, medium)),
            medium,
            &format!("shelf-{tag}"),
            &golden,
        );
        // The Report (view 09) on the same model: the header's verbs
        // over the chapter and the rail, at the still's width.
        #[allow(clippy::cast_precision_loss)]
        capture(
            Element::surface()
                .width(WIDTH as f32)
                .align_start()
                .child(ReportComposition::chips(medium))
                .child(
                    Element::surface()
                        .width(WIDTH as f32)
                        .height(400.0)
                        .child(ReportComposition::el(&shelved, medium)),
                ),
            medium,
            &format!("report-{tag}"),
            &golden,
        );
        // Birth: the ground card, the survey's columns over their field, the
        // interview's stack and the charting's list beside its field — each at the
        // still's width, the rooms at a height that holds them.
        #[allow(clippy::cast_precision_loss)]
        for (moment, model) in [
            ("ground", &ground),
            ("survey", &surveyed),
            ("interview", &interviewed),
            ("charting", &charted),
        ] {
            capture(
                Element::surface()
                    .width(WIDTH as f32)
                    .height(560.0)
                    .child(BirthComposition::el(
                        model,
                        medium,
                        (WIDTH as f32, 560.0),
                        CELL,
                    )),
                medium,
                &format!("birth-{moment}-{tag}"),
                &golden,
            );
        }
        capture(
            BriefComposition::el(&briefed, medium).expect("news"),
            medium,
            &format!("brief-{tag}"),
            &golden,
        );
        capture(
            MomentComposition::el(&asked, medium).expect("the ask"),
            medium,
            &format!("moment-ask-{tag}"),
            &golden,
        );
        capture(
            MomentComposition::el(&reviewing, medium).expect("the review"),
            medium,
            &format!("moment-review-{tag}"),
            &golden,
        );
    }
    assert!(
        failures.is_empty(),
        "Review candidates: {}",
        failures.join("\n")
    );
}
