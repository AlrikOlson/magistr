use magistr_cockpit::cockpit::{BirthMoment, Model, Msg, Place, Size, Widget, update};

use super::*;
use crate::comp_root::RootComposition;
use crate::element::Kind;
use crate::fixture::Fixture;

const SIZE: Size = Size::new(120, 40);
const ROOM: (f32, f32) = (1100.0, 589.0);

fn runs(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out.push(content.clone());
        }
    });
    out
}

fn ids_of(root: &Element) -> Vec<ElementId> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Some(id) = element.id {
            out.push(id);
        }
    });
    out
}

fn view(view: &str) -> Model {
    Fixture::mock(view, SIZE).expect("the flow has a fixture")
}

/// F1: the card's title and sub, the slot's copy, the two verbs and
/// the cancel cap; no header; the slot's wrapper and the verbs carry
/// their ids.
#[test]
fn the_ground_card_floats_with_its_slot_and_two_verbs() {
    let model = view("F1");
    assert_eq!(model.place(), Some(Place::Birth));
    assert_eq!(model.birth().moment(), BirthMoment::Ground);
    assert!(!BirthComposition::has_header(&model));
    let body = BirthComposition::el(&model, Medium::Night, ROOM, (9.0, 20.0));
    let texts = runs(&body);
    for copy in [
        "New project",
        "Select a folder, a repository, or start empty",
        "Drop a folder here",
        "Or click to browse",
        "Clone a repository\u{2026}",
        "Start empty",
        "esc \u{b7} Cancel",
    ] {
        assert!(texts.iter().any(|t| t == copy), "{copy}: {texts:?}");
    }
    let ids = ids_of(&body);
    for id in [
        BirthComposition::SLOT,
        BirthComposition::CLONE,
        BirthComposition::EMPTY,
    ] {
        assert!(ids.contains(&id), "{id:?}");
    }
    assert_eq!(
        RootComposition::target_of(&model, BirthComposition::SLOT),
        Some(Widget::BirthSlot.id())
    );
    assert_eq!(
        RootComposition::target_of(&model, BirthComposition::EMPTY),
        Some(Widget::BirthEmpty.id())
    );
    // A drag hovering arms the slot: the copy changes.
    let mut armed = model;
    update(&mut armed, Msg::DragHover(true));
    let armed_texts = runs(&BirthComposition::el(
        &armed,
        Medium::Night,
        ROOM,
        (9.0, 20.0),
    ));
    assert!(
        armed_texts.iter().any(|t| t == "Drop it here"),
        "{armed_texts:?}"
    );
    // A field being typed replaces the slot.
    update(&mut armed, Msg::DragHover(false));
    update(
        &mut armed,
        Msg::Mouse {
            event: magistr_cockpit::input::MouseEvent::new(
                magistr_cockpit::input::MouseKind::Down(magistr_cockpit::input::Button::Left),
                0,
                0,
            ),
            target: Some(Widget::BirthClone.id()),
        },
    );
    let field_texts = runs(&BirthComposition::el(
        &armed,
        Medium::Night,
        ROOM,
        (9.0, 20.0),
    ));
    assert!(
        field_texts.iter().any(|t| t == "Repository URL"),
        "{field_texts:?}"
    );
}

/// F2: the header chip says `Scanning…`, the four facts and four
/// receipts are the report's own words, the meter reads the facts
/// found, and the field is reserved for the sweep.
#[test]
fn the_survey_lands_the_reports_rows_and_reserves_the_field() {
    let model = view("F2");
    assert_eq!(model.birth().moment(), BirthMoment::Survey);
    assert!(BirthComposition::has_header(&model));
    assert_eq!(
        BirthComposition::header_height(&model),
        BirthComposition::HEADER_HEIGHT
    );
    let chips = runs(&BirthComposition::chips(&model, Medium::Night));
    assert_eq!(chips, vec!["Scanning\u{2026}"]);
    let body = BirthComposition::el(&model, Medium::Night, ROOM, (9.0, 20.0));
    let texts = runs(&body);
    for copy in [
        "Detected",
        "stack",
        "Rust \u{b7} edition 2024 \u{b7} cargo workspace, 4 crates",
        "agent",
        "claude-agent-acp on PATH",
        " Files written",
        "config",
        ".magistr/project.json",
        "Written",
        "Armed",
        "Opened",
        "Scan",
        "4 / 4",
    ] {
        assert!(texts.iter().any(|t| t == copy), "{copy}: {texts:?}");
    }
    assert!(ids_of(&body).contains(&BirthComposition::FIELD));
    let path: Vec<String> = model.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "new project \u{b7} ~/Code/tidewatch"]);
}

/// F3: the answered card carries the first answer in quotes, the
/// current card its cap, prompt, chips (two picked) and field, the
/// Continue verb and the `Next:` cap; the header's cap counts.
#[test]
fn the_interview_stacks_the_answered_card_over_the_question() {
    let model = view("F3");
    assert_eq!(model.birth().moment(), BirthMoment::Interview);
    let chips = runs(&BirthComposition::chips(&model, Medium::Day));
    assert_eq!(chips, vec!["2 of 3"]);
    let body = BirthComposition::el(&model, Medium::Day, ROOM, (9.0, 20.0));
    let texts = runs(&body);
    for copy in [
        "Answered",
        "What should tidewatch become?",
        "\u{201c}A tide-prediction service small harbors can self-host \u{2014} accurate enough to trust, cheap enough to ignore.\u{201d}",
        "Question 2 of 3",
        "What must never break?",
        "prediction accuracy \u{2713}",
        "the public API \u{2713}",
        "offline operation",
        "install under 5 min",
        "Type your own answer",
        "Continue",
        "Next: agent permissions \u{b7} 1 remaining",
    ] {
        assert!(texts.iter().any(|t| t == copy), "{copy}: {texts:?}");
    }
    let ids = ids_of(&body);
    assert!(ids.contains(&BirthComposition::CONTINUE));
    assert!(ids.contains(&ElementId(BirthComposition::CHIPS + 3)));
    assert_eq!(
        RootComposition::target_of(&model, ElementId(BirthComposition::CHIPS + 3)),
        Some(Widget::birth_chip(3))
    );
    let path: Vec<String> = model.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "new project \u{b7} tidewatch"]);
}

/// F4: the header carries the three verbs with Accept the one lit
/// exit, the list reads the proposal's rows and sources, the rows carry
/// their ids, the hints sit under the field, and the labels are born
/// with their stars.
#[test]
fn the_charting_lists_the_proposal_beside_its_field() {
    let model = view("F4");
    assert_eq!(model.birth().moment(), BirthMoment::Charting);
    assert_eq!(
        BirthComposition::header_height(&model),
        BirthComposition::VERBS_HEADER_HEIGHT
    );
    let chips = BirthComposition::chips(&model, Medium::Night);
    assert_eq!(
        runs(&chips),
        vec!["Accept this roadmap", "Redraw", "Edit chunks\u{2026}"]
    );
    assert_eq!(
        RootComposition::target_of(&model, BirthComposition::ACCEPT),
        Some(Widget::BirthAccept.id())
    );
    let body = BirthComposition::el(&model, Medium::Night, ROOM, (9.0, 20.0));
    let texts = runs(&body);
    for copy in [
        "Proposed \u{b7} 8 chunks \u{b7} 3 workstreams",
        "core \u{b7} harmonic engine",
        "ops \u{b7} harbor onboarding",
        "Sources",
        "Code scan \u{b7} interview answers",
        "Reorder",
        "Edit",
        "Accept",
    ] {
        assert!(texts.iter().any(|t| t == copy), "{copy}: {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t == "harmonic engine"),
        "at the first frame no star is born, so no label: {texts:?}"
    );
    let ids = ids_of(&body);
    assert!(ids.contains(&ElementId(BirthComposition::ROWS + 7)));
    assert_eq!(
        RootComposition::target_of(&model, ElementId(BirthComposition::ROWS + 7)),
        Some(Widget::birth_row(7))
    );
    // Born: every label is pinned.
    let mut born = model;
    for _ in 0..(Fixture::SETTLE_STEPS * 2) {
        update(&mut born, Msg::Tick);
    }
    let born_texts = runs(&BirthComposition::el(
        &born,
        Medium::Night,
        ROOM,
        (9.0, 20.0),
    ));
    for copy in ["harmonic engine", "station admin", "harbor onboarding"] {
        assert!(
            born_texts.iter().any(|t| t == copy),
            "{copy}: {born_texts:?}"
        );
    }
    let path: Vec<String> = born.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "tidewatch \u{b7} Proposed roadmap"]);
}

/// F5: the header counts the chunks, the field is the Sky's, the Ready
/// label names the first claim, the toast reads what was written and
/// offers Claim (the lit exit) and Later.
#[test]
fn first_light_is_the_sky_with_the_toast() {
    let model = view("F5");
    assert_eq!(model.birth().moment(), BirthMoment::FirstLight);
    assert!(model.birth_shows_sky());
    let chips = runs(&BirthComposition::chips(&model, Medium::Night));
    assert_eq!(chips, vec!["0 / 8 complete"]);
    let body = BirthComposition::el(&model, Medium::Night, ROOM, (9.0, 20.0));
    let texts = runs(&body);
    for copy in [
        "harmonic-engine \u{b7} Ready",
        "Roadmap created",
        "tidewatch",
        "8 chunks \u{b7} 3 workstreams \u{b7} Gates configured",
        "Claim harmonic-engine",
        "Later",
    ] {
        assert!(texts.iter().any(|t| t == copy), "{copy}: {texts:?}");
    }
    let ids = ids_of(&body);
    assert!(ids.contains(&crate::comp_sky::SkyComposition::FIELD));
    assert!(ids.contains(&BirthComposition::CLAIM));
    assert_eq!(
        RootComposition::target_of(&model, BirthComposition::LATER),
        Some(Widget::BirthLater.id())
    );
    assert_eq!(model.roadmap().rows().len(), 8, "the Sky is seeded");
    assert_eq!(model.title(), Some("tidewatch"));
    let path: Vec<String> = model.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "tidewatch"]);
}
