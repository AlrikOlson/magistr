use magistr_cockpit::cockpit::{Motion, Msg, Size, Surface, Widget, update};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

fn resting_sky() -> magistr_cockpit::cockpit::Model {
    let mut model = Fixture::with_pixels(Fixture::mock("03", Size::new(120, 40)).expect("view 03"));
    model.set_surface(Surface::Native);
    for _ in 0..(Motion::REST_STEPS + Motion::CHROME_FADE_STEPS) {
        update(&mut model, Msg::Tick);
    }
    model
}

/// At rest the chrome has exhaled: no counter, the field, and the
/// murmur — one row per chunk a session is on, in the star's own
/// state, off the roadmap the constellation draws.
#[test]
fn the_resting_sky_is_the_field_and_the_murmur() {
    let model = resting_sky();
    assert!(model.sky_at_rest());
    let sky = SkyComposition::el(&model, Medium::Night);
    let texts = texts(&sky);
    let chunk = model.roadmap().chunk("gui2-2a").expect("working item");
    assert!(texts.contains(&chunk.name), "the working item's title");
    assert!(
        texts.iter().any(|t| t == model.star_status_label(chunk)),
        "in its current state"
    );
    assert!(
        !texts.iter().any(|t| t == "211"),
        "the counter is awake chrome, not rest's"
    );
    let mut field = false;
    sky.each(&mut |_, element| field |= element.id == Some(SkyComposition::FIELD));
    assert!(field, "the constellation's field is reserved");
}

/// Intent wakes the room through the one `update`: the chrome is back
/// — the counter reads the rail's own progress — and the murmur is
/// rest's alone.
#[test]
fn intent_brings_the_chrome_back_and_stands_the_murmur_down() {
    let mut model = resting_sky();
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('x'))));
    assert!(!model.sky_at_rest(), "intent overrides rest at once");
    let sky = SkyComposition::el(&model, Medium::Night);
    let texts = texts(&sky);
    let (done, total) = model.rail().progress();
    assert!(texts.iter().any(|t| *t == done.to_string()), "the counter");
    assert!(texts.iter().any(|t| *t == format!(" / {total}")));
    let mut resting_row = false;
    sky.each(&mut |_, element| {
        resting_row |= element.owner == Some(crate::component::Component::StatusRow);
    });
    assert!(!resting_row, "the resting status row is gone");
    let card = magistr_cockpit::cockpit::Inspection::of(&model).expect("selected record");
    assert!(
        texts.contains(&card.status),
        "the selected record carries its current status"
    );
    assert!(model.roadmap().selected().is_some());
}

/// While the ship moment runs, the chrome carries the shipped cap
/// naming the star that ignited — [`Roadmap::ship_run`]'s own reading,
/// nothing re-derived.
///
/// [`Roadmap::ship_run`]: magistr_cockpit::cockpit::Roadmap::ship_run
#[test]
fn the_ship_moment_wears_the_shipped_cap() {
    let model = Fixture::with_pixels(Fixture::ship_08(Size::new(120, 40)));
    assert!(
        model.roadmap().ship_run(model.now()).is_some(),
        "the moment is in flight"
    );
    let sky = SkyComposition::el(&model, Medium::Night);
    assert!(
        texts(&sky).iter().any(|t| t == "shipped · gui2-2a"),
        "the cap names the star"
    );
}

/// A star answered the click, so the sky says what it is: the selected
/// chunk's card carries the status word, the id, the claim, the
/// acceptance and the verb the engines answer — folded through the one
/// shared update, exactly as a real click arrives.
#[test]
fn a_selected_star_tells_its_chunk_and_offers_the_engines_verb() {
    let mut model = Fixture::with_pixels(Fixture::mock("04", Size::new(120, 40)).expect("view 04"));
    model.set_surface(Surface::Native);
    let unselected = SkyComposition::el(&model, Medium::Night);
    assert!(
        !texts(&unselected).iter().any(|t| t.starts_with("Start ")),
        "no card before a click"
    );
    // Click the ready fixture chunk through the shared update function.
    let ready = (0..model.roadmap().dag_nodes().len())
        .find(|&i| {
            model
                .roadmap()
                .graph_chunk(i)
                .is_some_and(|c| c.state == magistr_cockpit::cockpit::ChunkState::Pending)
        })
        .expect("view 04 holds a claimable star");
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
            target: Some(Widget::sky_star(ready)),
        },
    );
    let chunk_id = model
        .roadmap()
        .selected()
        .expect("the click selected")
        .id
        .clone();
    let sky = SkyComposition::el(&model, Medium::Night);
    assert!(texts(&sky).contains(&chunk_id), "the id is on the card");
    assert!(
        texts(&sky).iter().any(|t| t == "Start"),
        "a claimable chunk offers Start beside its title"
    );
    let start = |root: &Element| {
        let mut found = false;
        root.each(&mut |_, element| {
            found |= element.id
                == Some(crate::comp_workspace::WorkspaceComposition::id(
                    Widget::RoadmapStart.id(),
                ))
        });
        found
    };
    assert!(start(&sky), "the verb wears its id");
}

/// The resting room keeps no card: a selection is chrome, and the
/// chrome has exhaled.
#[test]
fn the_resting_sky_keeps_no_selection_card() {
    let mut model = resting_sky();
    // Selecting any star stays on the Roadmap. After idle ticks, its
    // selection remains while the native chrome rests.
    if !model.roadmap().dag_nodes().is_empty() {
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
                target: Some(Widget::sky_star(0)),
            },
        );
        assert!(
            model.roadmap().selected().is_some(),
            "the selection is kept"
        );
        for _ in 0..(Motion::REST_STEPS + Motion::CHROME_FADE_STEPS) {
            update(&mut model, Msg::Tick);
        }
    }
    assert!(model.sky_at_rest(), "the room re-rested");
    let sky = SkyComposition::el(&model, Medium::Night);
    assert!(
        !texts(&sky)
            .iter()
            .any(|t| t.starts_with("Start ") || t.starts_with("Complete "))
    );
}
